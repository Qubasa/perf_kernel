use crate::pci::{Device, PciDevice, PCI_CONFIG_ADDRESS, PCI_CONFIG_DATA};
use alloc::sync::Arc;
use core::convert::TryInto;
use core::sync::atomic::compiler_fence;
use core::sync::atomic::Ordering;
use x86_64::instructions::port::Port;
use x86_64::{
    structures::paging::{
       FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};
#[derive(Debug, Copy, Clone)]
pub struct Rtl8139 {
    dev: PciDevice,
    addr: u32,
}

pub const BUF_START: usize = 0x_3333_3333_0000;
pub const BUF_SIZE: usize = 4096 * 3; // 100 KiB

impl Rtl8139 {
    pub fn new(dev: &PciDevice, addr: u32) -> Self {
        Self {
            dev: *dev,
            addr: addr,
        }
    }
    unsafe fn init(
        &self,
        mapper: &mut impl Mapper<Size4KiB>,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
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

        let page_range = {
            let heap_start = VirtAddr::new(BUF_START as u64);
            let heap_end = heap_start + BUF_SIZE - 1u64;
            let heap_start_page = Page::containing_address(heap_start);
            let heap_end_page = Page::containing_address(heap_end);

            Page::range_inclusive(heap_start_page, heap_end_page)
        };

        for page in page_range {
            let frame = frame_allocator
                .allocate_frame()
                .unwrap();
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE;
            unsafe {
                mapper
                    .map_to(page, frame, flags, frame_allocator)
                    .unwrap()
                    .flush()
            };
        }
        log::info!("Reset succeeded");
    }
}

impl Device for Rtl8139 {
    unsafe fn purge(&self) {}
}

pub fn probe(dev: &PciDevice, addr: u32) -> Option<Arc<dyn Device>> {
    if dev.header.vendor_id == 0x10EC && dev.header.device_id == 0x8139 {
        log::info!("Found pci device RTL8139");
        let device = Arc::new(Rtl8139::new(dev, addr));
        return Some(device);
    };
    return None;
}
