#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(asm)]
#![feature(test)]
#![feature(bench_black_box)]

use bootloader::bootinfo::MemoryRegionType;
use bootloader::mmu;
use bootloader::{acpi, bootinfo};
use bootloader::{klog::LOGGER, pagetable, smp};
use core::convert::TryInto;
use log::LevelFilter;

mod media_extensions;
use core::ptr::{addr_of, read_unaligned};
use multiboot2::MemoryAreaType;
use raw_cpuid::CpuId;
use smp::BOOT_INFO;

global_asm!(include_str!("multiboot2_header.s"));
global_asm!(include_str!("start.s"));
global_asm!(include_str!("smp_trampoline.s"));

type StackT = [u16; 0x1000 * 15];
#[no_mangle]
pub static STACK_SIZE: usize = core::mem::size_of::<StackT>();
#[no_mangle]
pub static mut STACK_ARRAY: StackT = [0; 0x1000 * 15];

/*
 * Important: The variables defined below are NOT pointers
 * to the section but usize slices of the section data itself.
 * To make it to an actual pointer get a reference of it.
 */
extern "C" {
    fn switch_to_long_mode(
        boot_info: &'static bootinfo::BootInfo,
        entry_point: u32,
        stack_addr: u32,
    ) -> !;
    static __bootloader_start: usize;
    static __smp_trampoline_start: usize;
    static __smp_trampoline_end: usize;
    static __identity_map_offset: usize;
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
    static _p1_tss_tables_start: usize;
    static _p1_tables_end: usize;
    static _p1_tables_start: usize;
    static __page_table_end: usize;
    static __minimum_mem_requirement: usize;
}

// TODO: We have some n^2 complexity checking in here
// we need a flame graph / execution hotspot map and start optimizing
// there. As I do not how well this scales if we have 2Tb+ of memory.
// I think it should be fine, nonetheless it should be looked after at some point
// IMPORTANT: TODO: Set a stack guard for the bootloader.
// Right now if the stack gets bigger then 4096*30 then we go below 1Mb
// where memory devices are mapped
// and we will get all kinds of undefined behavior
// TODO: If the bootloader reaches a size that exactly uses all usable space up between bootloader
// and kernel the bootloader/grub gets stuck.
// TODO: IMPORTANT: Do not print BOOT_INFO in bootloader as this will copy it to the stack and overflow it
// TODO: If a cpu starts with apic id 20 instead of 0 the bootloader fails right now.
#[no_mangle]
unsafe extern "C" fn bootloader_main(magic: u32, mboot2_info_ptr: u32) {
    // Needs to be here or else the linker does not include the
    // kernel. The symbol _kernel_size does not come from the linker script
    // but from objcopy. Read more under `$ man objcopy`
    core::hint::black_box(_kernel_size);

    // Initialization

    {
        bootloader::serial::init();
        bootloader::vga::init();

        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(LevelFilter::Info);

        // Load interrupt handlers for x86 mode
        bootloader::interrupts::init();

        // Checks multiboot2 magic
        if magic != multiboot2::MULTIBOOT2_BOOTLOADER_MAGIC {
            panic!(
                "EAX magic is incorrect. Booted from a non compliant bootloader: {:#x}",
                magic
            );
        }
        let cpuid = CpuId::new();
        // Normal checks
        if let Some(vf) = cpuid.get_vendor_info() {
            if !(vf.as_str() == "GenuineIntel" || vf.as_str() == "AuthenticAMD") {
                panic!("Processor is neither from Intel nor from AMD. {:?}", vf);
            }
        }
        let features = cpuid.get_feature_info().unwrap();
        if !features.has_msr() {
            panic!("Processor does not support MSR instructions");
        }
        if !features.has_pae() {
            panic!("Processor does not support PAE");
        }
        if !features.has_apic() {
            panic!("Processor does not support APIC interrupt controller");
        }
        if !features.has_pat() {
            panic!("Processor does not support memory page attributes");
        }

        log::info!(
            "CPU Family: {:x}h, CPU Model: {:x}h",
            features.family_id(),
            features.model_id()
        );
        // Extended feature checks
        let features = cpuid
            .get_extended_processor_and_feature_identifiers()
            .unwrap();
        if !features.has_64bit_mode() {
            panic!("Processor does not support x86_64 instruction set");
        }
        if !features.has_execute_disable() {
            panic!("Processor does not support NX bit");
        }
    }

    // Parses the multiboot2 header
    let parsed_multiboot_headers = match multiboot2::load(mboot2_info_ptr as usize) {
        Ok(i) => i,
        Err(e) => {
            panic!("Parsing multiboot header failed {:#?}", e);
        }
    };

    log::info!("name: {}", parsed_multiboot_headers.boot_loader_name_tag().unwrap().name() );
    log::info!("cmd: {}", parsed_multiboot_headers.command_line_tag().unwrap().command_line());

   for i in parsed_multiboot_headers.module_tags() {
       log::info!("boot module cmdline {}", i.cmdline());
   }

    // Save smp trampoline addr to BOOT_INFO
    BOOT_INFO.smp_trampoline = &__smp_trampoline_start as *const usize as u32;

    /*
     * Convert memory areas to memory map
     * also sums up detected RAM in byte
     */
    let mut existing_ram = 0; // All memory
    let mut available_ram = 0; // Memory that is tagged as 'available'
    {
        let map_tag = parsed_multiboot_headers.memory_map_tag().unwrap();
        for i in map_tag.all_memory_areas() {
            log::debug!("map tag: {:#x?}", i);
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

    // Fix overlapping memory ranges
    {
        let mut last: Option<&mut bootinfo::MemoryRegion> = None;
        for map in BOOT_INFO.memory_map.iter_mut() {
            if let Some(ref mut last) = last {
                if last.range.intersects(map.range.start_addr()) {
                    log::debug!("Memory maps intersect: \n {:#?} <-> {:#?}", last, map);
                    if read_unaligned(addr_of!(map.region_type))
                        == bootinfo::MemoryRegionType::Usable
                    {
                        map.range.set_start_addr(last.range.end_addr());
                        continue;
                    }

                    let max_end_addr = core::cmp::max(last.range.end_addr(), map.range.end_addr());
                    map.range.set_end_addr(max_end_addr);
                    last.range.set_end_addr(map.range.start_addr());
                }
            }
            last = Some(map);
        }
    }

    // Set all usable memory that is below 1Mib to UsableButDangerous
    {
        for map in BOOT_INFO.memory_map.iter() {
            if read_unaligned(addr_of!(map.region_type)) == bootinfo::MemoryRegionType::Usable {
                let start_addr = read_unaligned(addr_of!(map.range)).start_addr();
                let end_addr = read_unaligned(addr_of!(map.range)).end_addr();
                let one_mib = 0x100000; // 1Mib in hex
                if start_addr < one_mib && end_addr < one_mib {
                    BOOT_INFO
                        .memory_map
                        .partition_memory_region(
                            start_addr,
                            end_addr,
                            bootinfo::MemoryRegionType::UsableButDangerous,
                        )
                        .unwrap();
                } else if start_addr < one_mib && end_addr > one_mib {
                    BOOT_INFO
                        .memory_map
                        .partition_memory_region(
                            start_addr,
                            one_mib,
                            bootinfo::MemoryRegionType::UsableButDangerous,
                        )
                        .unwrap();
                }
            }
        }
    }

    // Checks that the current loaded image lies in available (good) physical memory
    {
        check(
            &__smp_trampoline_start as *const usize as u64,
            &__smp_trampoline_end as *const usize as u64,
        );
        check(
            &__bootloader_start as *const usize as u64,
            &__bootloader_end as *const usize as u64,
        );
        check(
            &__kernel_start as *const usize as u64,
            &__kernel_end as *const usize as u64,
        );
        check(
            &__page_table_start as *const usize as u64,
            &__page_table_end as *const usize as u64,
        );

        fn check(addr: u64, addr_end: u64) {
            if addr % 4096 != 0 {
                panic!("Addr is not page aligned: {:#?}", addr);
            }

            if addr_end <= addr {
                panic!("addr_end is smaller or equal to addr");
            }

            for addr in (addr..addr_end).step_by(4096) {
                unsafe {
                    if let Some(region) = BOOT_INFO.memory_map.get_region_by_addr(addr) {
                        let mem_type = read_unaligned(addr_of!(region.region_type));
                        if mem_type != MemoryRegionType::Usable
                            && mem_type != MemoryRegionType::UsableButDangerous
                        {
                            panic!(
                                "Part of loaded image lies in non usable memory! Addr: {:#x} with region: {:#?}",
                                addr,
                                region,
                            );
                        }
                    } else {
                        panic!("Region not in memory map. Unusable memorys");
                    }
                }
            }
        }
    }

    // Check that bootloader is not bigger then 1Mb
    if &__bootloader_end as *const _ as u64 >= bootloader::TWO_MEG {
        panic!("Bootloader is too big. The bootloader needs to fit between address 1Mb - 2Mb");
    }

    // Check if enough RAM available
    let min_ram = &__minimum_mem_requirement as *const _ as u64;
    if available_ram < min_ram {
        panic!("Kernel needs at least {} Kb of usable RAM", min_ram / 1024);
    }

    // Check that kernel lies at 2Mb in memory
    let start_addr = &__kernel_start as *const _ as u64;
    {
        if start_addr != bootloader::TWO_MEG {
            panic!(
                "Kernel start address needs to be 0x200000. Is however: {:#x}",
                start_addr
            );
        }
    }

    // Bootloader assumes that the apic_id of the BSP is 0
    if smp::apic_id() != 0 {
        panic!("BSP core is non zero. Bootloader did not expect that.");
    }

    // Map first 4Gb with 2Mb pages that are writable if memory is tagged usable
    // else pages are set readable with NX bit set.
    let p4_physical =
        mmu::generate_page_table(&_p4, &_p3, &_p2_tables_start, &_p2_tables_end, &BOOT_INFO);

    // Update MEM_MAP
    {
        BOOT_INFO
            .memory_map
            .partition_memory_region(0, 4096, bootinfo::MemoryRegionType::FrameZero)
            .unwrap();

        BOOT_INFO
            .memory_map
            .partition_memory_region(
                &__smp_trampoline_start as *const _ as u64,
                &__smp_trampoline_end as *const _ as u64,
                bootinfo::MemoryRegionType::SmpTrampoline,
            )
            .unwrap();

        BOOT_INFO
            .memory_map
            .partition_memory_region(
                &__bootloader_start as *const _ as u64,
                &__bootloader_end as *const _ as u64,
                bootinfo::MemoryRegionType::Bootloader,
            )
            .unwrap();

        BOOT_INFO
            .memory_map
            .partition_memory_region(
                &__kernel_start as *const _ as u64,
                &__kernel_end as *const _ as u64,
                bootinfo::MemoryRegionType::Kernel,
            )
            .unwrap();

        BOOT_INFO
            .memory_map
            .partition_memory_region(
                &__page_table_start as *const _ as u64,
                &__page_table_end as *const _ as u64,
                bootinfo::MemoryRegionType::PageTable,
            )
            .unwrap();
    }

    // Remap first 2Mb with 4Kb pages
    // sets stack guard page to read only
    // sets frame zero 0-4Kb to unmapped
    // also id maps vga address to uncachable
    mmu::remap_first_2mb_with_4kb(&_p3, &_p1_tables_start, &BOOT_INFO);

    // Allocate 8Mb stack space for every core
    // + 2 mb guard page at the end
    {
        use core::convert::TryFrom;
        let allocator = pagetable::BootInfoFrameAllocator::new(&BOOT_INFO.memory_map);
        let stack_size = bootloader::TWO_MEG * 4;
        let guard_page = bootloader::TWO_MEG;
        let mut iter = allocator.usable_xsize_frames(stack_size + guard_page, bootloader::TWO_MEG);

        // Generates an iterator to get Lapic structs on every next() call
        let lapic_iter = acpi::LapicIter::new().expect("Couldn't find acpi table");

        for (i, lapic) in lapic_iter.enumerate() {
            BOOT_INFO.cores.num_cores += 1;

            let addr = iter
                .next()
                .expect("Not enough memory to allocate stack for all cores");
            let stack_start = addr + stack_size + guard_page;
            BOOT_INFO.cores[i as usize].set_stack_start(stack_start.try_into().unwrap());
            BOOT_INFO.cores[i as usize].stack_end_addr = (addr + guard_page).try_into().unwrap();
            BOOT_INFO.cores[i as usize].set_apic_id(lapic.id);
            log::debug!(
                "Core {} stack space from: {:#x} to {:#x} with apic id: {}",
                i,
                addr + guard_page,
                stack_start,
                lapic.id
            );

            // Set 2Mb guard page to readable with NX bit set
            let p3_physical = &_p3 as *const _ as u64;
            let p3_table = &*(p3_physical as *mut pagetable::PageTable);
            let p3_index = usize::try_from(addr >> 12 >> 9 >> 9 & 0o777).unwrap();
            let p2_table = &mut *(p3_table[p3_index].addr() as *mut pagetable::PageTable);
            let p2_index = usize::try_from(addr >> 12 >> 9 & 0o777).unwrap();
            p2_table[p2_index].set_flags(
                pagetable::PageTableFlags::PRESENT
                    | pagetable::PageTableFlags::HUGE_PAGE
                    | pagetable::PageTableFlags::NO_EXECUTE,
            );

            // Mark guard page
            BOOT_INFO
                .memory_map
                .partition_memory_region(
                    addr,
                    addr + guard_page, // end addr
                    bootinfo::MemoryRegionType::GuardPage,
                )
                .unwrap();

            // Mark kernel stack
            BOOT_INFO
                .memory_map
                .partition_memory_region(
                    addr + guard_page, // start addr
                    stack_start,       // end addr
                    bootinfo::MemoryRegionType::KernelStack,
                )
                .unwrap();
        }
    }

    if BOOT_INFO.cores.num_cores > bootloader::MAX_CORES.try_into().unwrap() {
        panic!(
            "CPU has more then {} cores. Recompile with different MAX_CORES constant",
            bootloader::MAX_CORES
        );
    }

    if BOOT_INFO.cores.num_cores == 0 {
        panic!("Invalid value zero for MAX_CORES constant");
    }

    // Allocate eight 120KiB stacks + 8 KiB Guard Page per core for TSS
    // NOTE: If you want to move this into a separate function don't do it...yet.
    // We would need a copy/clone of the memory map for the FrameAllocator and this oversteps the stack
    // we first need a proper stack guard to catch these kinds of bugs in the bootloader
    {
        use core::convert::TryFrom;
        use pagetable::PageTable;
        use pagetable::PageTableFlags;
        let allocator = pagetable::BootInfoFrameAllocator::new(&BOOT_INFO.memory_map);
        let stack_size = 4096 * 30; // 120 KiB
        let guard_page = 4096 * 2; // 8 KiB

        // Create iterator that on every next() call returns a new mutable pde page table
        let mut p1_allocator =
            pagetable::PageTableAllocator::new(&_p1_tss_tables_start, &_p1_tables_end);
        let mut p1_table: Option<&'static mut PageTable> = None;
        let mut usable_frames = allocator
            .usable_xsize_frames(stack_size + guard_page, bootloader::TWO_MEG)
            .peekable();

        // Iter over number of cores
        for ci in 0..BOOT_INFO.cores.num_cores {
            let addr = *usable_frames
                .peek()
                .expect("Not enough memory to allocate tss stack for all cores");
            if ci % 2 == 0 {
                let p = p1_allocator
                    .next()
                    .expect("Not enough p1 tables in linker script allocated");
                p.zero();

                // Delete huge page entry and set pointer to p1 table
                {
                    // Make sure that addr is a new 2Mb page
                    if addr % bootloader::TWO_MEG != 0 {
                        panic!("Not 2Meg aligned: {:#x}", addr);
                    }

                    let p3_physical = &_p3 as *const _ as u64;
                    let p3_table = &*(p3_physical as *mut pagetable::PageTable);
                    let p3_index = usize::try_from(addr >> 12 >> 9 >> 9 & 0o777).unwrap();
                    let p2_table = &mut *(p3_table[p3_index].addr() as *mut pagetable::PageTable);
                    let p2_index = usize::try_from(addr >> 12 >> 9 & 0o777).unwrap();
                    p2_table[p2_index].set_addr(
                        addr_of!(*p) as u64,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    );
                }

                for (i, entry) in p.iter_mut().enumerate() {
                    let a = addr + i as u64 * 4096;
                    entry.set_addr(a, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
                }

                p1_table = Some(p);
            }

            #[allow(clippy::needless_option_as_deref)]
            let p1_table: &mut PageTable = p1_table.as_deref_mut().unwrap();

            // Iter over number of stacks for TSS
            for i in 0..8 {
                let addr = usable_frames
                    .next()
                    .expect("Not enough memory to allocate tss stack for all cores");
                let stack_start = addr + stack_size + guard_page;

                // Populate BOOT_INFO with stack addresses for every core
                BOOT_INFO.cores[ci as usize]
                    .tss
                    .set_stack_start(i as usize, stack_start.try_into().unwrap());
                BOOT_INFO.cores[ci as usize].tss.stack_end_addr[i as usize] =
                    (addr + guard_page).try_into().unwrap();

                // Compute p1 index of address
                let start_index = usize::try_from(addr >> 12 & 0o777).unwrap();

                // Iter over number of guard pages
                for (i, bytes) in (0..guard_page).step_by(4096).enumerate() {
                    p1_table[start_index + i].set_addr(
                        addr + bytes,
                        PageTableFlags::PRESENT | PageTableFlags::NO_EXECUTE,
                    );
                }

                // Mark guard page
                BOOT_INFO
                    .memory_map
                    .partition_memory_region(
                        addr,
                        addr + guard_page, // end addr
                        bootinfo::MemoryRegionType::GuardPage,
                    )
                    .unwrap();

                // Mark tss stack
                BOOT_INFO
                    .memory_map
                    .partition_memory_region(
                        addr + guard_page, // start addr
                        stack_start,       // end addr
                        bootinfo::MemoryRegionType::TSSstack,
                    )
                    .unwrap();
            }
        }
    }

    // Enable all media extensions
    media_extensions::enable_all();

    // Enable mmu
    // and load cr3 register with addr of page table
    mmu::setup_mmu(p4_physical);

    // Save start addr of page table to BOOT_INFO
    BOOT_INFO.page_table_addr = p4_physical.as_u32();

    // Add boot 0 to booted cores
    BOOT_INFO.cores.num_booted_cores += 1;

    // Check that kernel ELF header is correct
    let kernel_header = get_kernel_header(&__kernel_start);

    // We assume first apic id is 0. Get the stack for core 0
    let stack_addr: u32 = BOOT_INFO.cores[0]
        .get_stack_start()
        .expect("Forgot to instantiate kernel stack") -8;

    log::info!("BSP stack start: {:#x}", stack_addr);

    // Read kernel entry point from ELF header
    let entry_addr: u32 = kernel_header.e_entry;

    // Save entry point to BOOT_INFO
    BOOT_INFO.kernel_entry_addr = entry_addr;

    log::debug!("Switching to long mode...");

    // Switch to long mode and jump to kernel entry point
    switch_to_long_mode(&BOOT_INFO, entry_addr, stack_addr);
}

unsafe fn get_kernel_header(kernel_start: &'static usize) -> &Elf32Header {
    // Check that kernel ELF header is correct
    let kernel_header = core::mem::transmute::<&usize, &Elf32Header>(kernel_start);
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
    kernel_header
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Elf32Header {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u32,
    /*redacted*/
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use bootloader::println;
    println!("ERROR: {}", info);
    loop {}
}
