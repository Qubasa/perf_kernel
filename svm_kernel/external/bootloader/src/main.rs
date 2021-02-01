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
use bootloader::gdt;
use log::LevelFilter;
use multiboot2;
use multiboot2::MemoryAreaType;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr3, Cr3Flags, Cr4, Cr4Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging::frame::PhysFrame;
use x86_64::structures::paging::{MappedPageTable, LegacyOffsetPageTable, Mapper, Page, PageTableFlags, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};


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

static mut MEM_MAP: MemoryMap = MemoryMap::new();

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


    // Create a page table obj at addr _p4
    let mut mapper: LegacyOffsetPageTable = {
        let p4_addr = &_p4 as *const _ as u64;
        let phys_frame = PhysFrame::from_start_address(PhysAddr::new(p4_addr)).unwrap();
        Cr3::write(phys_frame, Cr3Flags::PAGE_LEVEL_WRITETHROUGH);
        // Offset is zero because everything is still in physical address space
        memory::init(VirtAddr::new(0))
    };

    // Zero the page table
    mapper.level_4_table().zero();

    // Create a 4KiB FrameAllocator instance
    let mut frame_allocator = memory::BootInfoFrameAllocator::new(&MEM_MAP);

    // Identity map the page table
    let page_table_start = &__page_table_start as *const _ as usize;
    let page_table_end = &__page_table_end as *const _ as usize;
    for i in (page_table_start..page_table_end).step_by(0x1000) {
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(i as u64));
        mapper.identity_map(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT, &mut frame_allocator).unwrap().flush();
    }

    // Identity map the bootloader
    let bootloader_start = &__bootloader_start as *const _ as usize;
    let bootloader_end = &__bootloader_end as *const _ as usize;
    for i in (bootloader_start..bootloader_end).step_by(0x1000) {
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(i as u64));
        mapper.identity_map(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT, &mut frame_allocator).unwrap().flush();
    }

    // Identity map the kernel
    let kernel_start = &_kernel_start_addr as *const _ as usize;
    let kernel_end = &_kernel_end_addr as *const _ as usize;
    for i in (kernel_start..kernel_end).step_by(0x1000) {
        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(i as u64));
        mapper.identity_map(frame, PageTableFlags::WRITABLE | PageTableFlags::PRESENT, &mut frame_allocator).unwrap().flush();
    }

    // Sync mapping to this point
    core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);


    log::info!("Enable PAE");
    // Enable PAE
    let mut cr4_data = Cr4::read();
    cr4_data.set(Cr4Flags::PHYSICAL_ADDRESS_EXTENSION, true);
    Cr4::write(cr4_data);

    log::info!("Enable long mode");
    // Set long mode bit in efer
    let mut efer = Efer::read();
    efer.set(EferFlags::LONG_MODE_ACTIVE, true);
    Efer::write(efer);


    log::info!("Enable paging1");
    // Enable paging
    let mut cr0 = Cr0::read();
    cr0.set(Cr0Flags::PAGING, true);
    Cr0::write(cr0);

    log::info!("Load 64bit GDT");
    // Load 64bit GDT
    gdt::init();


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
