#![allow(unused_imports)]
#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};
use modular_bitfield::prelude::*;
use x86_64::instructions::port::Port;
use x86_64::registers::model_specific::Msr;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PhysFrame, Size4KiB,
};
use x86_64::VirtAddr;

// Other constants
const APIC_BASE: u64 = 0x0_0000_FEE0_0000;

// Offset the PICs to avoid index collision with
// exceptions in the IDT
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;
struct Pic {
    /// The base offset to which our interrupts are mapped.
    offset: u8,

    /// The processor I/O port on which we send commands.
    command: Port<u8>,

    /// The processor I/O port on which we send and receive data.
    data: Port<u8>,

    /// Backup of original mask value
    orig_mask: Option<u8>,
}
struct ChainedPics {
    pics: [Pic; 2],
}
impl ChainedPics {
    /// Create a new interface for the standard PIC1 and PIC2 controllers,
    /// specifying the desired interrupt offsets.
    pub const unsafe fn new(offset1: u8, offset2: u8) -> ChainedPics {
        ChainedPics {
            pics: [
                Pic {
                    offset: offset1,
                    command: Port::new(0x20),
                    data: Port::new(0x21),
                    orig_mask: None,
                },
                Pic {
                    offset: offset2,
                    command: Port::new(0xA0),
                    data: Port::new(0xA1),
                    orig_mask: None,
                },
            ],
        }
    }
}

/// APIC registers (offsets into MMIO space)
#[derive(Clone, Copy)]
#[repr(usize)]
pub enum Register {
    /// APIC ID register
    ApicId = 0x20,

    // Spurious Interrupt Vector Register
    SpurInterVecReg = 0xF0,
    ApicVersion = 0x30,
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
    res2: B12
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct LocalVecTableReg {
    vec: B8,
    message_type: B3,
    res0: B1,
    delivery_status: B1,
    res1: B1,
    remote_irr: B1,
    trigger_mode: B1,
    mask: B1,
    timer_mode: B1,
    res2: B14
}



pub struct Apic {
    apic_base_reg: Msr,
    chained_pics: ChainedPics,
    version: Option<ApicVersion>,
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

    // Disable chained pics
    unsafe fn disable_pics(&mut self) {
        let pic0 = &mut self.chained_pics.pics[0];
        pic0.orig_mask = Some(pic0.data.read());
        pic0.data.write(0xff);

        let pic1 = &mut self.chained_pics.pics[1];
        pic1.orig_mask = Some(pic1.data.read());
        pic1.data.write(0xff);
    }

    unsafe fn map_apic_page(
        &self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        use x86_64::{
            structures::paging::{FrameAllocator, Mapper, Page, PhysFrame, Size4KiB},
            PhysAddr,
        };
        let page = Page::<Size4KiB>::from_start_address(VirtAddr::new(APIC_BASE)).unwrap();

        // Map page for apic base address
        use x86_64::structures::paging::PageTableFlags as Flags;
        let frame = PhysFrame::containing_address(PhysAddr::new(APIC_BASE));
        let flags = Flags::PRESENT | Flags::WRITABLE | Flags::NO_CACHE | Flags::NO_EXECUTE;
        let map_to_result = mapper.map_to(page, frame, flags, frame_allocator).unwrap();

        // Flush TLB
        map_to_result.flush();
    }

    pub unsafe fn initialize(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) {
        // Disable old chained pics controller
        self.disable_pics();

        // Map page for apic base address
        self.map_apic_page(mapper, frame_allocator);

        // Enable apic
        let mut base_reg = ApicBaseReg::from_bytes(self.apic_base_reg.read().to_le_bytes());
        base_reg.set_apic_enable(1);
        base_reg.set_bootstrap_core(1);
        base_reg.set_apic_base_addr(0xfee00);
        let base_reg = u64::from_le_bytes(base_reg.into_bytes());
        self.apic_base_reg.write(base_reg);

        // Map spurious interrupts to index 0xff
        self.write_apic(Register::SpurInterVecReg, 0xff);

        // Read, parse & save apic version
        let apic_version = self.read_apic(Register::ApicVersion);
        self.version = Some(ApicVersion::from_bytes(apic_version.to_le_bytes()));
        log::info!(
            "APIC version: {}, max lvt entries: {}, extendend: {}",
            self.version.unwrap().ver(),
            self.version.unwrap().max_lvt_entries(),
            self.version.unwrap().extended_apic_space(),
        );
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

    pub fn apic_id(&self) -> u32 {
        unsafe { self.read_apic(Register::ApicId) }
    }
}
