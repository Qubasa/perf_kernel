use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use core::convert::TryInto;
use core::mem::size_of;
use spin;

/// Trait which allows for converting to an Any
pub trait AsAny {
    fn as_any(&self) -> &(dyn Any + 'static);
    fn as_any_mut(&mut self) -> &mut (dyn Any + 'static);
}

/// Implement AsAny for any T that implements Plugin and 'static
impl<T: Device + 'static> AsAny for T {
    /// Convert a reference of a Plugin to a reference to an Any
    fn as_any(&self) -> &(dyn Any + 'static) {
        self
    }

    /// Convert mutable reference of a Plugin to a mutable reference to an Any
    fn as_any_mut(&mut self) -> &mut (dyn Any + 'static) {
        self
    }
}

/// An driver for a device. There are multiple instances of a driver for each
/// device the driver handled during the probe process.
pub trait Device: Send + Sync  {
    /// Invoked on a device when we're doing a soft reboot. This may be called
    /// from an exceptionally hostile environment (eg. inside of a panic inside
    /// of an NMI exception). The goal of this function for a driver is to
    /// attempt to reset the device to state which will not disrupt the system
    /// during the soft reboot process.
    ///
    /// When a soft reboot happens, things like DMA and interrupts on any
    /// device should absolutely be turned off. Resetting the device fully in
    /// many situations is ideal. The device should be set to a state that
    /// shortly after the new kernel is reloaded, the device can be
    /// re-initialized with the standard device probe process.
    ///
    /// This will be invoked on the device regardless of locks, thus the
    /// device needs to be able to handle that a previous user of the device
    /// may have been interrupted mid-use.
    unsafe fn purge(&self);
}
type ProbeFunction = fn(&PciDevice, addr: u32) -> Option<()>;

/// List of all driver probe routines on the system. If they return `Some` then
/// we successfully found a driver and thus we'll register it in the
/// `DEVICES` database
const DRIVERS: &[ProbeFunction] = &[];

/// If `true` verbose PCI device enumeration will be displayed
const DEBUG_PCI_DEVICES: bool = false;

/// I/O port for the PCI configuration space window address
pub const PCI_CONFIG_ADDRESS: u16 = 0xcf8;

/// I/O port for the PCI configuration space window data
pub const PCI_CONFIG_DATA: u16 = 0xcfc;

/// Enable bit for accessing the `0xcf8` I/O port
const PCI_ADDRESS_ENABLE: u32 = 1 << 31;

pub static DEVICES: spin::Mutex<Vec<Arc<()>>> = spin::Mutex::new(Vec::new());

/// Common PCI header for the PCI configuration space of any device or bridge
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PciHeader {
    pub vendor_id: u16,
    pub device_id: u16,
    pub command: u16,
    pub status: u16,
    pub revision: u8,
    pub prog_if: u8,
    pub subclass: u8,
    pub class: u8,
    pub cache_line_size: u8,
    pub latency_timer: u8,
    pub header_type: u8,
    pub bist: u8,
}

/// Configuration space for a PCI device
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PciDevice {
    /// Standard PCI configuration space header
    pub header: PciHeader,

    pub bar0: u32,
    pub bar1: u32,
    pub bar2: u32,
    pub bar3: u32,
    pub bar4: u32,
    pub bar5: u32,
    pub cardbus_cis_pointer: u32,
    pub subsystem_vendor_id: u16,
    pub subsystem_device_id: u16,
    pub expansion_rom_address: u32,
    pub capabilities: u8,
    pub reserved: [u8; 7],
    pub interrupt_line: u8,
    pub interrupt_pin: u8,
    pub min_grant: u8,
    pub max_latency: u8,
}

pub unsafe fn init() {
    use x86_64::instructions::port::Port;
    let mut config_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
    let mut config_data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

    // Bitmap of present PCI devices
    let mut pci_enum = [0u64; 256 * 32 * 8 / 64];

    // For each possible bus ID
    for bus in 0..256 {
        // For each possible device ID
        for device in 0..32 {
            // For each possible function ID
            for function in 0..8 {
                // Compute the address to select this BDF combination
                let pci_addr = (bus << 8) | (device << 3) | (function << 0);

                // Compute the PCI selection address
                let addr = PCI_ADDRESS_ENABLE | (pci_addr << 8);

                // Select the address and read the device and vendor ID
                config_port.write(addr);
                let did_vid = config_data_port.read();

                if did_vid != 0xffff_ffff {
                    // Set the device present in the PCI enumeration table
                    let idx = pci_addr / 64;
                    let bit = pci_addr % 64;
                    pci_enum[idx as usize] |= 1 << bit;
                }
            }
        }
    }

    let pci_devices = pci_enum;

    for (idx, &pci_map) in pci_devices.iter().enumerate() {
        // No devices here, go to the next `u64`
        if pci_map == 0 {
            continue;
        }

        // There is at least 1 device in this set
        for bit in 0..64 {
            // If the bit is not set, no device here, skip it
            if (pci_map & (1u64 << bit)) == 0 {
                continue;
            }

            // Compute the PCI address for this bit
            let pci_addr = (idx * 64) | bit;

            // Compute the PCI selection address
            let addr = PCI_ADDRESS_ENABLE | (pci_addr << 8) as u32;

            // Read the PCI configuration header
            let mut header = [0u32; size_of::<PciHeader>() / size_of::<u32>()];
            for (rid, register) in header.iter_mut().enumerate() {
                // Set the window to the register we want to read and read the
                // value
                config_port.write(addr | (rid * size_of::<u32>()) as u32);
                *register = config_data_port.read();
            }

            // Convert the header to our `PciHeader` structure
            let header: PciHeader = core::ptr::read_unaligned(header.as_ptr() as *const PciHeader);

            // Skip non-device PCI entries (skips things like PCI bridges)
            if (header.header_type & 0x7f) != 0 {
                continue;
            }

            // Read the PCI configuration
            let mut device = [0u32; size_of::<PciDevice>() / size_of::<u32>()];
            for (rid, register) in device.iter_mut().enumerate() {
                // Set the window to the register we want to read and read the
                // value
                config_port.write(addr | (rid * size_of::<u32>()) as u32);
                *register = config_data_port.read();
            }

            // Convert the device to our `PciDevice` structure
            let device: PciDevice = core::ptr::read_unaligned(device.as_ptr() as *const PciDevice);

            if DEBUG_PCI_DEVICES {
                log::info!(
                    "PCI device | {:#06x}:{:#06x} | {:#06x}:{:#06x}\n",
                    device.header.vendor_id,
                    device.header.device_id,
                    device.subsystem_vendor_id,
                    device.subsystem_device_id
                );
            }

            // Attempt to find a driver for this device
            for probe in DRIVERS {
                if let Some(_driver) = probe(&device, addr.try_into().unwrap()) {
                    // Found a handler, go to the next function during the PCI
                    // enumeration
                    //DEVICES.lock().push(driver);
                    //TODO: Replace () with a driver
                }
            }
        }
    }
}
