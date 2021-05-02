use crate::pci::{Device, PciDevice, PCI_CONFIG_ADDRESS, PCI_CONFIG_DATA};
use alloc::sync::Arc;
use core::convert::TryFrom;
use core::convert::TryInto;
use core::sync::atomic::compiler_fence;
use core::sync::atomic::Ordering;
use x86_64::instructions::port::Port;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{FrameAllocator, Mapper, Size2MiB};
#[derive(Debug, Copy, Clone)]
pub struct Rtl8139 {
    dev: PciDevice,
    addr: u32,
}

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
            let data: u32 = (self.dev.header.status as u32) << 16 | command as u32;
            config_data_port.write(data);
        }

        let test = config_data_port.read();
        if (test as u16) != self.dev.header.command | (1 << 2) {
            panic!("This should be the same");
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
        crate::memory::id_map_nocache_update_flags(mapper, frame_allocator, frame, None).unwrap();
        log::info!("Reset succeeded");

        let mut rbstart: Port<u32> = Port::new((iobase + 0x30).try_into().unwrap());
        rbstart.write(
            u32::try_from(frame.start_address().as_u64())
                .expect("Frame allocator allocated frame outside of 4Gb range"),
        );

        let mut imr: Port<u16> = Port::new((iobase + 0x3C).try_into().unwrap());
        imr.write(0x5);  // Sets the TOK and ROK bits high

        let mut rcr: Port<u32> = Port::new((iobase + 0x44).try_into().unwrap());
        rcr.write(0xf | (1<<7)); // (1 << 7) is the WRAP bit, 0xf is AB+AM+APM+AAP

        // Enable receiver and transmitter
        cmd.write(0xc);
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
