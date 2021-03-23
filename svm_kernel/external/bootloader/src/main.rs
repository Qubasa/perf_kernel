#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(asm)]
#![allow(unused_imports)]

use bootloader::bootinfo;
use bootloader::bootinfo::MemoryRegionType;
use bootloader::mylog::LOGGER;
use bootloader::pagetable;
use core::convert::TryInto;
use log::LevelFilter;
use multiboot2;
use multiboot2::MemoryAreaType;
use x86::structures::gdt::*;
use x86::structures::paging::frame::PhysFrame;
use x86::{PhysAddr, VirtAddr};


global_asm!(include_str!("boot.s"));
global_asm!(include_str!("start.s"));


/*
 * Important: The variables defined below are NOT pointers
 * to the section but usize slices of the section data itself.
 * To make it to an actual pointer get a reference of it.
 */
extern "C" {
    fn switch_to_long_mode(boot_info: &'static bootinfo::MemoryMap, entry_point: *const usize);
    static __bootloader_start: usize;
    static __offset: usize;
    static __stack_end: usize;
    static __stack_start: usize;
    static init_bootloader: usize;
    static __bootloader_end: usize;
    static _kernel_start_addr: usize;
    static _kernel_end_addr: usize;
    static __page_table_start: usize;
    static _p4: usize;
    static _p3: usize;
    static _p2_tables_start: usize;
    static _p2_tables_end: usize;
    static __page_table_end: usize;
}

static mut MEM_MAP: bootinfo::MemoryMap = bootinfo::MemoryMap::new();

#[no_mangle]
unsafe extern "C" fn bootloader_main(magic: u32, mboot2_info_ptr: u32) {
    #[allow(non_snake_case)]
    let PHYS_MEM_OFFSET = &__offset as *const _ as u64;

    // Initialization
    {
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(LevelFilter::Debug);
        log::info!(
            "Bootloader start addr: {:#?}",
            &__bootloader_start as *const _
        );
        log::info!("Bootloader main addr:  {:#?}", &bootloader_main as *const _);
        log::info!("Bootloader init addr:  {:#?}", &init_bootloader as *const _);
        log::info!("Stack start:  {:#?}", &__stack_start as *const _);
        log::info!("Stack end:  {:#?}", &__stack_end as *const _);
        let esp: u32;
        asm!("mov {}, esp", out(reg) esp);
        log::info!("ESP:  {:#x}", esp);


        // Load interrupt handlers for x86 mode
        bootloader::interrupts::load_idt();

        // Checks multiboot2 magic
        if magic != 0x36d76289 {
            panic!(
                "EAX magic is incorrect. Booted from a non compliant bootloader: {:#x}",
                magic
            );
        }
    }

    // Parses the multiboot2 header
    let boot_info = multiboot2::load(mboot2_info_ptr as usize);

    /*
     * Convert memory areas to memory map
     * also sums up detected RAM in byte
     */
    let mut existing_ram = 0; // All memory
    let mut available_ram = 0; // Memory that is tagged as 'available'
    {
        let map_tag = boot_info.memory_map_tag().unwrap();
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
        log::info!("Existing ram: {} Kib", existing_ram / 1024);

        // Sums up usable ram
        for i in map_tag.memory_areas() {
            available_ram += i.size();
        }
        log::info!("Available ram: {} Kib", available_ram / 1024);
        log::info!(
            "Ram overhead: {} KiB",
            (existing_ram - available_ram) / 1024
        );
    }

    // Checks that the current loaded image lies in available (good) physical memory
    {
        for i in MEM_MAP.iter() {
            check(&i, __bootloader_start as *const usize as u64);
            check(&i, __bootloader_end as *const usize as u64);
            check(&i, _kernel_start_addr as *const usize as u64);
            check(&i, _kernel_end_addr as *const usize as u64);
            check(&i, __page_table_start as *const usize as u64);
            check(&i, __page_table_end as *const usize as u64);
        }

        fn check(region: &bootloader::bootinfo::MemoryRegion, addr: u64) {
            if region.range.intersects(addr) {
                if region.region_type != MemoryRegionType::Usable {
                    panic!(
                        "Part of loaded image lies in non usable memory! Addr: {:#x}",
                        addr
                    );
                }
            }
        }
    }

    // Check if enough RAM available
    if available_ram < bootloader::ONE_MEG * 512 {
        panic!("Kernel needs at least 512Mb of RAM");
    }

    // Check if paging already enabled
    {
        use x86::registers::control::Cr0;
        use x86::registers::control::Cr0Flags;
        let cr0 = Cr0::read();
        if cr0.contains(Cr0Flags::PAGING) {
            panic!("Paging has already been enabled by bootloader, this is unexpected. Use Bios load and not Efi load");
        }
    }

    // Generate page table for long mode
    let p4_physical = &_p4 as *const _ as u32;
    let p4_physical = PhysAddr::new(p4_physical);
    {
        let p4_table = &mut *(p4_physical.as_u32() as *mut pagetable::PageTable);
        p4_table.zero();

        // Every entry in p4 is 512Gb big in total p4 can do 512Gb*512 entries = 256Tb
        // Every entry in p3 is   1Gb big in total p3 can do 1G*512    entries = 512Gb
        // Every entry in p2 is   2Mb big in total p2 can do 2M*512    entries = 1Gb
        // Every entry in p1 is   4Kb big in total p1 can do 4K*512    entries = 2Mb
        // Memory requirements for first 4Gb mapped with 4Kb pages
        // 4*(8*512*512*512) = 4G (lol)
        // Memory requirements for first 4Gb mapped with 2Mb pages
        // 4*(8*512*512) = 8Mb
        let p3_physical = &_p3 as *const _ as u32;
        let p3_physical = PhysAddr::new(p3_physical);
        let mut entry = pagetable::PageTableEntry::new();
        entry.set_addr(
            p3_physical,
            pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
        );
        p4_table[0] = entry;

        // Populate p3 table with 2Mb pages
        let p3_table = &mut *(p3_physical.as_u32() as *mut pagetable::PageTable);
        let mut frame_finder =
            pagetable::BootInfoFrameAllocator::new(&MEM_MAP).usable_2m_frames(PHYS_MEM_OFFSET);

        let mut pde_allocator = pagetable::PdeAllocator::new(&_p2_tables_start, &_p2_tables_end);

        // Map first 2Gb
        for pdpe_i in 0..2 {
            let mut entry = pagetable::PageTableEntry::new();
            let pde: &'static mut pagetable::PageTable = pde_allocator
                .next()
                .expect("Not enough space for another p2 table");

            for (pde_i, entry) in pde.iter_mut().enumerate() {
                let virt_addr =
                    pdpe_i as u64 * bootloader::ONE_GIG + pde_i as u64 * bootloader::TWO_MEG;

                // Do not map memory below phys mem offset
                if pdpe_i == 0 && virt_addr < PHYS_MEM_OFFSET {
                    continue;
                }

                let phys_addr = frame_finder.next().expect("Not enough available memory");
                // log::info!("Mapping {:#x} to {:#x}", virt_addr, phys_addr);
                if virt_addr != phys_addr {
                    panic!("Identity mapping failed");
                }
                entry.set_addr(
                    PhysAddr::new(
                        phys_addr
                            .try_into()
                            .expect("phys addr outside of 32bit range"),
                    ),
                    pagetable::PageTableFlags::PRESENT
                        | pagetable::PageTableFlags::WRITABLE
                        | pagetable::PageTableFlags::HUGE_PAGE,
                );
            }
            let pde_addr = core::mem::transmute::<&'static mut pagetable::PageTable, u32>(pde);
            entry.set_addr(
                PhysAddr::new(pde_addr),
                pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
            );
            p3_table[pdpe_i] = entry;
        }
    }
    //TODO: Update MEM_MAP
    //TODO: Change set_addr to accept u64 instead of PhysAddr u32
    //TODO: Check if kernel bigger then available memory
    //TODO: Map memory variably
    //TODO: Check that this is an AMD cpu
    log::info!("Done creating page table.");

    // Load P4 to CR3 register
    {
        use x86::registers::control::{Cr3, Cr3Flags};
        let (_, flags) = Cr3::read();
        Cr3::write(PhysFrame::from_start_address(p4_physical).unwrap(), flags);
    }

    switch_to_long_mode(&MEM_MAP, 0 as *const usize);

    loop {}
}

#[allow(dead_code)]
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

use x86::structures::paging::page_table::{PageTable, PageTableFlags};
pub unsafe fn dump_page_table(page_table_ptr: &PageTable, pae: bool) {
    for i in 0..512 {
        let entry = &page_table_ptr[i];
        if entry.is_unused() {
            continue;
        }

        let page_table_ptr = &*(entry.addr().as_u32() as *const PageTable);
        let ps = entry.flags().contains(PageTableFlags::HUGE_PAGE);
        if pae == true && ps == true {
            panic!("Grub pagetable uses 2Mb pages");
        } else if pae == false && ps == false {
            // 4Kib pages
            let mut last_kb = 0;
            let mut last_addr = PhysAddr::new(0);
            for z in 0..512 {
                let entry = &page_table_ptr[z];
                if entry.is_unused() {
                    continue;
                }
                if last_addr == entry.addr() {
                    continue;
                }
                bootloader::println!(
                    "{:#x} - {}Mb {}<->{}Kb Mapped to: {:#?}",
                    i * 4 * 1024 * 1024 + z * 4 * 1024,
                    i * 4,
                    last_kb,
                    z * 4,
                    entry.addr()
                );
                if z * 4 - last_kb == 512 || z * 4 - last_kb == 1024 {
                    panic!(" You sure this is not a 4Mb or 2Mb page?");
                }
                last_kb = z * 4;
                last_addr = entry.addr();
            }
        } else if pae == false && ps == true {
            bootloader::println!(
                "{:#x} - {}Mb Mapped to: {:#?}",
                i * 4 * 1024 * 1024,
                i * 4,
                entry.addr()
            );
        }
    }
}
