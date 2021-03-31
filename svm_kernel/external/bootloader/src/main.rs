#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(asm)]
#![feature(test)]
#![allow(unused_imports)]

use bootloader::bootinfo;
use bootloader::bootinfo::MemoryRegionType;
use bootloader::mylog::LOGGER;
use bootloader::pagetable;
use core::convert::TryInto;
use log::LevelFilter;
use multiboot2;
mod media_extensions;
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
    fn switch_to_long_mode(boot_info: &'static bootinfo::BootInfo, entry_point: u64) -> !;
    static __bootloader_start: usize;
    static __identity_map_offset: usize;
    static __stack_end: usize;
    static __stack_start: usize;
    static _start_bootloader: usize;
    static __bootloader_end: usize;
    static __kernel_start: usize;
    static _kernel_size: usize;
    static __kernel_end: usize;
    static __page_table_start: usize;
    static _p4: usize;
    static _p3: usize;
    static _p2_tables_start: usize;
    static _p2_tables_end: usize;
    static __page_table_end: usize;
    static __minimum_mem_requirement: usize;
}

static mut BOOT_INFO: bootinfo::BootInfo = bootinfo::BootInfo::new();

#[no_mangle]
unsafe extern "C" fn bootloader_main(magic: u32, mboot2_info_ptr: u32) {

    // Needs to be here or else the linker does not include the
    // kernel. The symbol _kernel_size does not come from the linker script
    // but from objcopy. Read more under `$ man objcopy`
    core::hint::black_box(_kernel_size);

    // Initialization
    {
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(LevelFilter::Debug);

        // Load interrupt handlers for x86 mode
        bootloader::interrupts::load_idt();

        // Checks multiboot2 magic
        if magic != 0x36d76289 {
            panic!(
                "EAX magic is incorrect. Booted from a non compliant bootloader: {:#x}",
                magic
            );
        }

        // Checks that this is a x64 processor
        use core::arch::x86::__cpuid;

        let res = __cpuid(0x8000_0001);

        if res.edx & (1 << 29) == 0 {
            panic!("Processor does not support x86_64 instruction set");
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
            BOOT_INFO.memory_map.add_region(region);
        }
        BOOT_INFO.max_phys_memory = existing_ram;
        log::info!("Existing Ram: {} KiB", existing_ram / 1024);

        // Sums up usable ram
        for i in map_tag.memory_areas() {
            available_ram += i.size();
        }
        log::info!(
            "Unusable Ram: {} KiB",
            (existing_ram - available_ram) / 1024
        );
    }

    // Checks that the current loaded image lies in available (good) physical memory
    {
        for i in BOOT_INFO.memory_map.iter() {
            check(&i, &__stack_start as *const usize as u64);
            check(&i, &__stack_end as *const usize as u64);
            check(&i, &__bootloader_start as *const usize as u64);
            check(&i, &__bootloader_end as *const usize as u64);
            check(&i, &__kernel_start as *const usize as u64);
            check(&i, &__kernel_end as *const usize as u64);
            check(&i, &__page_table_start as *const usize as u64);
            check(&i, &__page_table_end as *const usize as u64);
        }

        fn check(region: &bootloader::bootinfo::MemoryRegion, addr: u64) {
            if region.range.intersects(addr) {
                if region.region_type != MemoryRegionType::Usable {
                    panic!(
                        "Part of loaded image lies in non usable memory! Addr: {:#x} with region: {:#?}",
                        addr,
                        region,
                    );
                }
            }
        }
    }

    if &__bootloader_end as *const _ as u64 >= bootloader::TWO_MEG {
        panic!("Bootloader is too big. The bootloader needs to fit between address 1Mb - 2Mb");
    }

    // Check if enough RAM available
    let min_ram = &__minimum_mem_requirement as *const _ as u64;
    if available_ram < min_ram {
        panic!("Kernel needs at least {}Kb of usable RAM", min_ram / 1024);
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
        let p3_physical = &_p3 as *const _ as u64;
        let mut entry = pagetable::PageTableEntry::new();
        entry.set_addr(
            p3_physical,
            pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
        );
        p4_table[0] = entry;

        // Populate p3 table with 2Mb pages
        let p3_table = &mut *(p3_physical as *mut pagetable::PageTable);
        // let mut frame_finder = pagetable::BootInfoFrameAllocator::new(&BOOT_INFO.memory_map)
        //     .usable_2m_frames(PHYS_MEM_OFFSET);

        let mut pde_allocator = pagetable::PdeAllocator::new(&_p2_tables_start, &_p2_tables_end);

        // Identity map first 4Gb
        for pdpe_i in 0..4 {
            let mut entry = pagetable::PageTableEntry::new();
            let pde: &'static mut pagetable::PageTable = pde_allocator
                .next()
                .expect("Not enough space for another p2 table");

            for (pde_i, entry) in pde.iter_mut().enumerate() {
                let virt_addr =
                    pdpe_i as u64 * bootloader::ONE_GIG + pde_i as u64 * bootloader::TWO_MEG;

                let phys_addr = virt_addr;
                use bootinfo::MemoryRegionType;

                // TODO: Make more efficient with a search algo
                let flags =
                    if let Some(mem_area) = BOOT_INFO.memory_map.get_region_by_addr(phys_addr) {
                        match mem_area.region_type {
                            MemoryRegionType::Usable => {
                                pagetable::PageTableFlags::PRESENT
                                    | pagetable::PageTableFlags::WRITABLE
                                    | pagetable::PageTableFlags::HUGE_PAGE
                            }
                            MemoryRegionType::BadMemory => {
                                continue;
                            }
                            _ => {
                                pagetable::PageTableFlags::PRESENT
                                    | pagetable::PageTableFlags::HUGE_PAGE
                                    | pagetable::PageTableFlags::NO_EXECUTE
                            }
                        }
                    } else {
                        continue;
                    };
                entry.set_addr(phys_addr, flags);
            }
            let pde_addr = core::mem::transmute::<&'static mut pagetable::PageTable, u32>(pde);
            entry.set_addr(
                pde_addr as u64,
                pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
            );
            p3_table[pdpe_i] = entry;
        }
    }

    // Update MEM_MAP
    {
        BOOT_INFO.memory_map.partition_memory_region(
            &__kernel_start as *const _ as u64,
            &__kernel_end as *const _ as u64,
            bootinfo::MemoryRegionType::Kernel,
        ).unwrap();

        BOOT_INFO.memory_map.partition_memory_region(
            &__bootloader_start as *const _ as u64,
            &__bootloader_end as *const _ as u64,
            bootinfo::MemoryRegionType::Bootloader,
        ).unwrap();

        BOOT_INFO.memory_map.partition_memory_region(
            &__stack_end as *const _ as u64,
            &__stack_start as *const _ as u64,
            bootinfo::MemoryRegionType::KernelStack,
        ).unwrap();

        BOOT_INFO.memory_map.partition_memory_region(
            &__page_table_start as *const _ as u64,
            &__page_table_end as *const _ as u64,
            bootinfo::MemoryRegionType::PageTable,
        ).unwrap();
    }

    //TODO: Enable sse
    media_extensions::enable_all();

    // Enable write protection CR0 bit
    {
        use x86::registers::control::{Cr0, Cr0Flags};
        let mut flags = Cr0::read();
        flags.set(Cr0Flags::WRITE_PROTECT, true);
        Cr0::write(flags);
    }

    // Enable no execute bit
    {
        use x86::registers::model_specific::{Efer, EferFlags};
        let mut flags = Efer::read();
        flags.set(EferFlags::NO_EXECUTE_ENABLE, true);
        Efer::write(flags);
    }

    //TODO: Enable floating point
    log::debug!("Done creating page table.");

    // Load P4 to CR3 register
    {
        use x86::registers::control::{Cr3, Cr3Flags};
        let (_, flags) = Cr3::read();
        Cr3::write(PhysFrame::from_start_address(p4_physical).unwrap(), flags);
    }

    let kernel_header = core::mem::transmute::<&usize, &Elf32Header>(&__kernel_start);
    let magic = [
        0x7f, 0x45, 0x4c, 0x46, 0x02, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];
    if kernel_header.e_ident != magic {
        for i in kernel_header.e_ident.iter() {
            bootloader::print!("{:#x} ", i);
        }
        panic!("\n Invalid ELF header magic of kernel!");
    }

    let start_addr = &__kernel_start as *const _ as u64;
    if start_addr != bootloader::TWO_MEG {
        panic!(
            "Kernel start address needs to be 0x200000. Is however: {:#x}",
            start_addr
        );
    }

    log::debug!("Switching to long mode...");

    let entry_addr = kernel_header.e_entry as u64;
    switch_to_long_mode(&BOOT_INFO, entry_addr);
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Elf32Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u32,
    /*redacted*/
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
