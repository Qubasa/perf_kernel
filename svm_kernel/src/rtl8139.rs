use crate::pci::{Device, PciDevice, PCI_CONFIG_ADDRESS, PCI_CONFIG_DATA};
use alloc::sync::Arc;
use core::convert::TryFrom;
use core::convert::TryInto;
use core::sync::atomic::compiler_fence;
use core::sync::atomic::Ordering;
use modular_bitfield::prelude::*;
use x86_64::addr::VirtAddr;
use x86_64::instructions::port::Port;
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size2MiB};

#[derive(Debug, Clone)]
pub struct Rtl8139 {
    dev: PciDevice,
    addr: u32,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct TransmitStatusReg {
    pub size: B13,
    pub own: B1,
    pub tun: B1,
    pub tok: B1,
    pub ertxth: B6,
    pub resv0: B2,
    pub ncc: B4,
    pub cdh: B1,
    pub owc: B1,
    pub tabt: B1,
    pub crs: B1,
}

static mut CURR_REG: usize = 0;
static mut TRANSMIT_REGS: [Option<Port<u32>>; 4] = [None; 4];
static mut TRANSMIT_ADDR: [Option<VirtAddr>; 4] = [None; 4];
static mut STATUS_REGS: [Option<Port<u32>>; 4] = [None; 4];

const MAX_RECV_BUFFER: u64 = 4096 * 3; // Required are only 9708 but for alignment reasons
const MAX_TRANS_BUFFER: u64 = 4096; // Required are only 1792

impl Rtl8139 {
    pub fn new(dev: &PciDevice, addr: u32) -> Self {
        Self {
            dev: *dev,
            addr: addr,
        }
    }
    pub unsafe fn init(
        &self,
        mapper: &mut (impl Mapper<Size2MiB> + Translate),
        frame_allocator: &mut (impl FrameAllocator<Size2MiB>
                  + FrameAllocator<x86_64::structures::paging::Size4KiB>),
    ) {
        let mut config_port: Port<u32> = Port::new(PCI_CONFIG_ADDRESS);
        let mut config_data_port: Port<u32> = Port::new(PCI_CONFIG_DATA);

        config_port.write(self.addr | 0x4);

        let test = config_data_port.read();
        if (test as u16) != self.dev.header.command {
            panic!("This should be the same");
        }

        if self.dev.header.command & (1 << 2) == 0 {
            log::info!("Making device to bus master...");
            let command = self.dev.header.command | (1 << 2);
            let data: u32 = command as u32;
            config_data_port.write(data);
        }

        let test = config_data_port.read();
        if (test as u16) != self.dev.header.command | (1 << 2) {
            panic!("This should be the same");
        }

        if self.dev.header.command & (1 << 10) != 0 {
            panic!("Interrupts are disabled");
        }

        if self.dev.header.command & (1 << 0) == 0 {
            panic!("I/O space is disabled");
        }

        let bar0 = self.dev.bar0;
        if bar0 & 1 == 0 {
            panic!("This driver should map to I/O space not to memory");
        }

        compiler_fence(Ordering::SeqCst);

        let iobase = bar0 & (!0b11);
        let mut config_1: Port<u8> = Port::new((iobase + 0x52).try_into().unwrap());
        config_1.write(0x0);

        compiler_fence(Ordering::SeqCst);

        let mut cmd: Port<u8> = Port::new((iobase + 0x37).try_into().unwrap());
        cmd.write(0x10); // reset
        let mut data = cmd.read();
        while (data & 0x10) != 0 {
            data = cmd.read();
        }

        let frame = frame_allocator.allocate_frame().unwrap();
        log::info!("Base frame allocated: {:?}", frame);
        crate::memory::id_map_nocache_update_flags(
            mapper,
            frame_allocator,
            frame,
            Some(PageTableFlags::WRITABLE),
        )
        .unwrap();
        log::info!("Reset succeeded");

        let mut rbstart: Port<u32> = Port::new((iobase + 0x30).try_into().unwrap());
        rbstart.write(
            u32::try_from(frame.start_address().as_u64())
                .expect("Frame allocator allocated frame outside of 4Gb range"),
        );

        STATUS_REGS[0] = Some(Port::new((iobase + 0x10).try_into().unwrap()));
        STATUS_REGS[1] = Some(Port::new((iobase + 0x14).try_into().unwrap()));
        STATUS_REGS[2] = Some(Port::new((iobase + 0x18).try_into().unwrap()));
        STATUS_REGS[3] = Some(Port::new((iobase + 0x1C).try_into().unwrap()));

        TRANSMIT_REGS[0] = Some(Port::new((iobase + 0x20).try_into().unwrap()));
        TRANSMIT_REGS[1] = Some(Port::new((iobase + 0x24).try_into().unwrap()));
        TRANSMIT_REGS[2] = Some(Port::new((iobase + 0x28).try_into().unwrap()));
        TRANSMIT_REGS[3] = Some(Port::new((iobase + 0x2C).try_into().unwrap()));

        for (i, reg) in TRANSMIT_REGS.iter_mut().enumerate() {
            let port = reg.as_mut().unwrap();
            let addr: u32 =
                (frame.start_address().as_u64() + MAX_RECV_BUFFER + (i as u64 * MAX_TRANS_BUFFER))
                    .try_into()
                    .unwrap();
            TRANSMIT_ADDR[i] = Some(VirtAddr::new(addr as u64));
            port.write(addr);
        }

        let mut imr: Port<u16> = Port::new((iobase + 0x3C).try_into().unwrap());
        imr.write(0x5); // Sets the TOK and ROK bits high

        let mut rcr: Port<u32> = Port::new((iobase + 0x44).try_into().unwrap());
        rcr.write(0xf | (1 << 7)); // (1 << 7) is the WRAP bit, 0xf is AB+AM+APM+AAP

        // Enable receiver and transmitter
        cmd.write(0xc);

        log::info!("Interrupt line is: {}", self.dev.interrupt_line);
        log::info!("Interrupt pin: {}", self.dev.interrupt_pin);

        if self.dev.interrupt_line != 11 {
            panic!("The interrupt line has been hardcoded for this CTF, please do not use more then one pci device");
        }
    }

    pub fn enable_receive_packet(&self) {
        let iobase = self.dev.bar0 & (!0b11);
        let mut intr: Port<u16> = Port::new((iobase + 0x3E).try_into().unwrap());
        unsafe {
            intr.write(0x1); // clears the Rx OK bit
        };
    }

    pub unsafe fn send(&self, data: &[u8]) {
        let status_reg = STATUS_REGS[CURR_REG].as_mut().unwrap();
        let addr = TRANSMIT_ADDR[CURR_REG].unwrap();

        log::info!("Copying to buffer {:#x} curr reg: {}", addr, CURR_REG);
        core::ptr::copy_nonoverlapping(data.as_ptr(), addr.as_u64() as *mut u8, data.len());

        compiler_fence(Ordering::SeqCst);

        let status = TransmitStatusReg::new()
            .with_own(0)
            .with_size(data.len().try_into().unwrap());
        status_reg.write(u32::from_le_bytes(status.into_bytes()));

        compiler_fence(Ordering::SeqCst);

        let mut done = status_reg.read();
        while done & (1 << 15) == 0 {
            done = status_reg.read();
        }
        log::info!("Successfully send packet");

        if CURR_REG >= 3 {
            CURR_REG = 0;
        } else {
            CURR_REG += 1;
        }
    }
}

impl Device for Rtl8139 {
    unsafe fn purge(&self) {}
}

pub fn probe(dev: &PciDevice, addr: u32) -> Option<Arc<Rtl8139>> {
    if dev.header.vendor_id == 0x10EC && dev.header.device_id == 0x8139 {
        log::info!("Found pci device RTL8139");
        let device = Arc::new(Rtl8139::new(dev, addr));
        return Some(device);
    };
    return None;
}
