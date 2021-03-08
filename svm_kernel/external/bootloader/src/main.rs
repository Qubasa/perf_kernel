#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(asm)]

#[allow(unused_imports)]
use bootloader::bootinfo;
use bootloader::bootinfo::MemoryRegionType;
use bootloader::mylog::LOGGER;
use log::LevelFilter;
use multiboot2;
use multiboot2::MemoryAreaType;
use x86::registers::control::{Cr0, Cr0Flags, Cr3, Cr3Flags, Cr4, Cr4Flags};
use x86::registers::model_specific::{Efer, EferFlags};
use x86::structures::paging::frame::PhysFrame;
use x86::structures::paging::{MappedPageTable, Mapper, Page, PageTableFlags, Size4KiB};
use x86::{PhysAddr, VirtAddr};


global_asm!(include_str!("boot.s"));
global_asm!(include_str!("start.s"));
extern "C" {
    static _kernel_start_addr: usize;
    static _kernel_end_addr: usize;
    static _p4: usize;
    static __page_table_start: usize;
    static __page_table_end: usize;
    static __bootloader_start: usize;
    static __bootloader_end: usize;
}

static mut MEM_MAP: bootinfo::MemoryMap = bootinfo::MemoryMap::new();

#[no_mangle]
unsafe extern "C" fn bootloader_main(magic: u32, mboot2_info_ptr: u32) {
    // Enable & set log level
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Debug);
    log::info!("Bootloader main.");

    // Checks multiboot2 magic
    if magic != 0x36d76289 {
        panic!(
            "EAX magic is incorrect. Booted from a non compliant bootloader: {:#x}",
            magic
        );
    }

    // Parses the multiboot2 header
    let boot_info =  multiboot2::load(mboot2_info_ptr as usize);

    /*
     * Convert memory areas to memory map
     * also sums up detected RAM in KiB
     */
    let map_tag = boot_info.memory_map_tag().unwrap();
    let mut existing_ram = 0;
    for i in map_tag.all_memory_areas() {
        existing_ram += i.size();

        let region = bootinfo::MemoryRegion {
            range: bootinfo::FrameRange::new(i.start_address(), i.end_address()),
            region_type: match i.typ() {
                MemoryAreaType::Reserved => MemoryRegionType::Reserved,
                MemoryAreaType::Available => MemoryRegionType::Usable,
                MemoryAreaType::AcpiAvailable => MemoryRegionType::AcpiReclaimable,
                MemoryAreaType::ReservedHibernate => MemoryRegionType::AcpiNvs,
                MemoryAreaType::Defective => MemoryRegionType::BadMemory,
            },
        };

        log::debug!(
            "Frame {:#x} - {:#x} Type: {:#?}",
            i.start_address(),
            i.end_address(),
            i.typ()
        );
        MEM_MAP.add_region(region);
    }
    existing_ram = existing_ram / 1024;
    log::info!("Existing ram: {} Kib", existing_ram);

    // Sums up usable ram
    let mut available_ram = 0;
    for i in map_tag.memory_areas() {
        available_ram += i.size();
    }
    available_ram = available_ram / 1024;
    log::info!("Available ram: {} Kib", available_ram);
    log::info!("Ram overhead: {} KiB", existing_ram - available_ram);

    // Check if huge pages are supported
    if !supports_gb_pages() {
        panic!("Current CPU does not support 1GiB pages");
    }

    log::info!("looping now");
    loop {}
}

fn supports_gb_pages() -> bool {
    use core::arch::x86::__cpuid;

    let res = unsafe { __cpuid(0x8000_0001) };

    if res.edx & (1 << 26) == 0 {
        return false;
    }
    return true;
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use bootloader::println;
    println!("ERROR: {}", info);
    loop {}
}
