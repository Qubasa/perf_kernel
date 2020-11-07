#![allow(dead_code)]

use acpi::{AcpiHandler, AcpiTables, PhysicalMapping};
use core::ptr::{read_volatile, write_volatile};
use modular_bitfield::prelude::*;
use pic8259_simple::ChainedPics;
use x86_64::registers::model_specific::Msr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

// Other constants
const APIC_BASE: u64 = 0x0_0000_FEE0_0000;
// https://stackoverflow.com/questions/24828186/about-the-io-apic-82093aa
const IO_APIC_BASE: u64 = 0xFEC00000;

// Offset the PICs to avoid index collision with
// exceptions in the IDT
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// IDT index numbers
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    LegacyTimer = PIC_1_OFFSET,
    Keyboard, // 33
    Reserved0,
    COM2,
    COM1,
    IRQ5,
    FloppyController,
    ParallelPort1,
    RtcTimer,
    ACPI,
    ScsiNic1,
    ScsiNic2,
    Mouse,
    MathCoProcessor,
    AtaChannel1,
    AtaChannel2,
    Timer = 0xe0,
    Spurious = 0xff,
}

impl InterruptIndex {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

/// APIC registers (offsets into MMIO space)
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Register {
    ApicId = 0x20,
    SpurInterVecReg = 0xF0,
    ApicVersion = 0x30,
    EndOfInterrupt = 0xB0,
    ApicTimer = 0x320,
    TimerCurrentCount = 0x390,
    TimerInitialCount = 0x380,
    DivideConfReg = 0x3E0,
    TaskPrioReg = 0x80,
    DestFormatReg = 0xE0,
    LogicalDestReg = 0xD0,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct DestFormatReg {
    res: B28,
    model: B4,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct LogicalDestReg {
    res: B24,
    dest_logical_id: B8,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct TaskPrioReg {
    task_prio: B4,
    task_prio_subclass: B4,
    res0: B24,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct ApicVersion {
    ver: B8,
    res0: B8,
    max_lvt_entries: B8,
    res1: B7,
    extended_apic_space: B1,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct ApicBaseReg {
    res0: B8,
    bootstrap_core: B1,
    res1: B2,
    apic_enable: B1,
    apic_base_addr: B40,
    res2: B12,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct DivideConfReg {
    div: B2,
    rev0: B1,
    div2: B1,
    res1: B28,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct TimerLvtReg {
    vec: B8,
    res0: B4,
    delivery_status: B1,
    res1: B3,
    mask: B1,
    timer_mode: B1,
    res2: B14,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct SpuriousInterReg {
    vec: B8,
    apic_enable: B1,
    fcc: B1,
    res0: B22,
}

//TODO: Save orig apic state to
// make a soft reboot possible
pub struct Apic {
    apic_base_reg: Msr,
    chained_pics: ChainedPics,
    version: Option<ApicVersion>,
}

use alloc::rc::Rc;
use core::cell::RefCell;
#[derive(Clone)]
pub struct MyAcpiHandler<'a> {
    mapper: Rc<RefCell<&'a mut OffsetPageTable<'a>>>,
    frame_allocator: Rc<RefCell<&'a mut dyn FrameAllocator<Size4KiB>>>,
}

impl<'a> MyAcpiHandler<'a> {
    pub fn new(
        mapper: &'a mut OffsetPageTable<'a>,
        frame_allocator: &'a mut impl FrameAllocator<Size4KiB>,
    ) -> Self {
        MyAcpiHandler {
            mapper: Rc::new(RefCell::new(mapper)),
            frame_allocator: Rc::new(RefCell::new(frame_allocator)),
        }
    }
}

impl AcpiHandler for MyAcpiHandler<'_> {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let page =
            Page::<Size4KiB>::from_start_address(VirtAddr::new(physical_address as u64)).unwrap();
        use x86_64::structures::paging::PageSize;
        if size > Size4KiB::SIZE as usize {
            panic!("Size is bigger then a 4k Page");
        }

        // Map page for apic base address
        use x86_64::structures::paging::PageTableFlags as Flags;
        let frame = PhysFrame::containing_address(PhysAddr::new(physical_address as u64));
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_CACHE | Flags::NO_EXECUTE;
        let map_to_result = self
            .mapper
            .get_mut()
            .map_to(page, frame, flags, self.frame_allocator.get_mut().clone())
            .unwrap();

        // Flush TLB
        map_to_result.flush();

        PhysicalMapping {
            physical_start: frame.start_address().as_u64() as usize,
            virtual_start: core::ptr::NonNull::new(page.start_address().as_mut_ptr()).unwrap(),
            region_length: size,
            mapped_length: page.size() as usize,
            handler: self.clone(),
        }
    }
    fn unmap_physical_region<T>(&self, region: &PhysicalMapping<Self, T>) {
        todo!();
    }
}

impl Apic {
    pub const fn new() -> Self {
        let chained = unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) };
        Apic {
            apic_base_reg: Msr::new(0x0000_001B),
            chained_pics: chained,
            version: None,
        }
    }

    unsafe fn map_apic_page(
        &self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        let page = Page::<Size4KiB>::from_start_address(VirtAddr::new(APIC_BASE)).unwrap();

        // Map page for apic base address
        use x86_64::structures::paging::PageTableFlags as Flags;
        let frame = PhysFrame::containing_address(PhysAddr::new(APIC_BASE));
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_CACHE | Flags::NO_EXECUTE;
        let map_to_result = mapper.map_to(page, frame, flags, frame_allocator).unwrap();

        // Flush TLB
        map_to_result.flush();
    }

    fn is_supported(&self) -> bool {
        use core::arch::x86_64::__cpuid;
        let feature = unsafe { __cpuid(0x0000_0001) };
        let feature = feature.edx & (1 << 9);
        return feature != 0;
    }

    unsafe fn parse_madt(&self) {
        let x = APIC_BASE & 0xf << 2;
        let y = APIC_BASE & 3;

        log::info!("x: {:x}, y: {:x}", x, y);
        log::info!("IO BASE: 0xFEC0{:x}{:x}10", x, y);
        let apic_start = (IO_APIC_BASE + 10) as *const u8;

        // let magic = core::slice::from_raw_parts(apic_start, 4);
        let f = read_volatile(apic_start);
        log::info!("MADT magic: {:#x}", f);
        let f = read_volatile(apic_start.offset(1));
        log::info!("MADT magic: {:#x}", f);
        let f = read_volatile(apic_start.offset(2));
        log::info!("MADT magic: {:#x}", f);
        let f = read_volatile(apic_start.offset(3));
        log::info!("MADT magic: {:#x}", f);
    }

    fn parse_apic<'a>(
        &self,
        mapper: &'a mut OffsetPageTable<'a>,
        frame_allocator: &'a mut impl FrameAllocator<Size4KiB>,
    ) {
        let apic = MyAcpiHandler::new(mapper, frame_allocator);
    }

    pub unsafe fn initialize<'a>(
        &mut self,
        mapper: &'a mut OffsetPageTable<'a>,
        frame_allocator: &'a mut impl FrameAllocator<Size4KiB>,
    ) {
        if !self.is_supported() {
            panic!("Apic is not available");
        }

        // Initialize pic to set interrupt offsets
        // Needed because osdev.org said so
        self.chained_pics.initialize();

        // Disable old chained pics controller
        self.chained_pics.disable();

        // Map page for apic base address
        self.map_apic_page(mapper, frame_allocator);

        self.parse_apic(mapper, frame_allocator);

        self.parse_madt();

        // Enable apic by writing MSR base reg
        let mut base_reg = ApicBaseReg::from_bytes(self.apic_base_reg.read().to_le_bytes());
        base_reg.set_apic_enable(1);
        base_reg.set_apic_base_addr(0xfee00);
        let base_reg = u64::from_le_bytes(base_reg.into_bytes());
        self.apic_base_reg.write(base_reg);

        // Map spurious interrupts to index
        // and set apic enable bit
        let spur_vec = SpuriousInterReg::new()
            .with_vec(InterruptIndex::Spurious.as_u8())
            .with_apic_enable(1)
            .with_fcc(0);
        let spur_vec = u32::from_le_bytes(spur_vec.into_bytes());
        self.write_apic(Register::SpurInterVecReg, spur_vec);

        // Read, parse & save apic version
        let apic_version = self.read_apic(Register::ApicVersion);
        self.version = Some(ApicVersion::from_bytes(apic_version.to_le_bytes()));
        log::info!(
            "APIC version: {}, max lvt entries: {}, extendend: {}",
            self.version.unwrap().ver(),
            self.version.unwrap().max_lvt_entries(),
            self.version.unwrap().extended_apic_space(),
        );

        // Allow all interrupts
        self.write_apic(Register::TaskPrioReg, 0);

        self.init_timer();
    }

    unsafe fn init_timer(&self) {
        // Divide by two
        let div = DivideConfReg::new().with_div(0).with_div2(0);
        let div = u32::from_le_bytes(div.into_bytes());
        self.write_apic(Register::DivideConfReg, div);

        // clear mask & periodic timer
        let timer = TimerLvtReg::new()
            .with_vec(InterruptIndex::Timer.as_u8())
            .with_delivery_status(0)
            .with_mask(0)
            .with_timer_mode(1); // Periodic timer inters
        let timer = u32::from_le_bytes(timer.into_bytes());
        self.write_apic(Register::ApicTimer, timer);

        // Calculate this on every cpu anew
        // by measuring the time with a different clock
        let one_ms = 423845;
        self.write_apic(Register::TimerInitialCount, one_ms * 1000);
    }

    unsafe fn read_apic(&self, register: Register) -> u32 {
        let offset = register as u64;
        let ptr = (APIC_BASE + offset) as *mut u32;
        read_volatile(ptr)
    }

    unsafe fn write_apic(&self, register: Register, value: u32) {
        let offset = register as u64;
        let ptr = (APIC_BASE + offset) as *mut u32;
        write_volatile(ptr, value);
    }

    // TODO: Gamozo used this without needing to aquire a lock
    pub unsafe fn end_of_interrupt(&self) {
        self.write_apic(Register::EndOfInterrupt, 0);
    }

    pub fn apic_id(&self) -> u32 {
        unsafe { self.read_apic(Register::ApicId) }
    }
}
