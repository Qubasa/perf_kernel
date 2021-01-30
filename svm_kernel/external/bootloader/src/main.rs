#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(asm)]

#[allow(unused_imports)]
use bootloader::bootinfo::FrameRange;
use bootloader::bootinfo::MemoryMap;
use bootloader::bootinfo::MemoryRegion;
use bootloader::bootinfo::MemoryRegionType;
use bootloader::memory;
use bootloader::mylog::LOGGER;
use log::LevelFilter;
use multiboot2;
use multiboot2::MemoryAreaType;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr3, Cr3Flags, Cr4, Cr4Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging::frame::PhysFrame;
use x86_64::structures::paging::page::{Page, PageSize, Size1GiB, Size2MiB, Size4KiB};
use x86_64::structures::paging::LegacyOffsetPageTable;
use x86_64::{PhysAddr, VirtAddr};

global_asm!(include_str!("boot.s"));
global_asm!(include_str!("start.s"));
extern "C" {
    static _kernel_start_addr: usize;
    static _kernel_end_addr: usize;
    static _p4: usize;
}

#[no_mangle]
extern "C" fn bootloader_main(magic: u32, mboot2_info_ptr: u32) {
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
    let boot_info = unsafe { multiboot2::load(mboot2_info_ptr as usize) };

    // Saves all the physical memory ranges with tags
    let mut mem_map = MemoryMap::new();

    /*
     * Convert memory areas to memory map
     * also sums up detected RAM in KiB
     */
    let map_tag = boot_info.memory_map_tag().unwrap();
    let mut existing_ram = 0;
    for i in map_tag.all_memory_areas() {
        existing_ram += i.size();

        let region = MemoryRegion {
            range: FrameRange::new(i.start_address(), i.end_address()),
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
        mem_map.add_region(region);
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

    // Create a page table obj at addr _p4
    let mut mapper: LegacyOffsetPageTable = unsafe {
        let p4_addr = &_p4 as *const _ as u64;
        let phys_frame = PhysFrame::from_start_address(PhysAddr::new(p4_addr)).unwrap();
        Cr3::write(phys_frame, Cr3Flags::PAGE_LEVEL_WRITETHROUGH);
        // Offset is zero because everything is still in physical address space
        memory::init(VirtAddr::new(0))
    };

    // Zero the page table
    mapper.level_4_table().zero();

    // Create a 4KiB FrameAllocator instance
    let mut frame_allocator = unsafe { memory::BootInfoFrameAllocator::new(mem_map) };

    // Enable PAE
    let mut cr4_data = Cr4::read();
    cr4_data.set(Cr4Flags::PHYSICAL_ADDRESS_EXTENSION, true);
    unsafe {
        Cr4::write(cr4_data);
    }

    // Set long mode bit in efer
    let mut efer = Efer::read();
    efer.set(EferFlags::LONG_MODE_ACTIVE, true);
    unsafe {
        Efer::write(efer);
    }

    // Enable paging
    let mut cr0 = Cr0::read();
    cr0.set(Cr0Flags::PAGING, true);
    unsafe {
        Cr0::write(cr0);
    }

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
