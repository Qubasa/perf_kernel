#![allow(dead_code)]

use crate::acpi::Acpi;
use crate::apic_regs::*;
use crate::interrupts::InterruptIndex;
use crate::interrupts::PICS;
use core::ptr::{read_volatile, write_volatile};
use x86_64::registers::model_specific::Msr;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PhysFrame, Size4KiB};
use x86_64::PhysAddr;
use x86_64::structures::paging::page_table::PageTableFlags;

// Other constants
const APIC_BASE: u64 = 0x0_0000_FEE0_0000;

//TODO: Save orig apic state to
// make a soft reboot possible
#[derive(Clone, Copy, Debug)]
pub struct Apic {
    pub version: Option<ApicVersion>,
    pub id: Option<u8>,
    bsp: Option<bool>,
}

impl Apic {
    pub const fn new() -> Self {
        Apic {
            version: None,
            id: None,
            bsp: None,
        }
    }

    #[inline]
    pub fn is_bsp(&self) -> bool {
        return self.bsp.unwrap();
    }

    pub unsafe fn mp_init(&self, apic_id: u8, trampoline: u32) {
        log::info!("Trampoline ptr: {:#x}", trampoline);
        log::info!("Booting core {}", apic_id);
        // Send INIT ipi
        let low = InterCmdRegLow::new()
            .with_vec(0) // INIT needs vec to be zero
            .with_trigger_mode(0) // level-sensitive
            .with_msg_type(0b101) // INIT type
            .with_level(0) // 0 for INIT
            ;
        let high = InterCmdRegHigh::new().with_dest(apic_id);
        self.send_ipi(&low, &high);

        // Convert func pointer to u64
        let trampoline = trampoline as u64;

        // Check if trampoline in first MB
        if trampoline >= 0x100_000 {
            panic!("Trampoline is outside the 1MB reachable space");
        }

        // Check that we can safely shift the pointer 12 bits to the right
        if trampoline & 0xfff != 0 {
            panic!("Trampoline address must have lower 12 bits set to zero");
        }

        // Convert trampoline func pointer to u8
        let to_vec = (trampoline >> 12) as u8;

        if to_vec >= 0xA0 && to_vec <= 0xBF {
            panic!("Trampoline vector can't use 0xA0-0xBF. Reserved by spec.");
        }

        // Send STARTUP ipi
        let low = InterCmdRegLow::new()
            .with_vec(to_vec) // Core execute code at 0x000VV000
            .with_trigger_mode(0) // level-sensitive
            .with_msg_type(0b110) // STARTUP type
            .with_level(1) // 1 for everything else
            ;
        let high = InterCmdRegHigh::new().with_dest(apic_id);
        self.send_ipi(&low, &high);
    }

    #[inline]
    fn ipi_pending(&self) -> bool {
        unsafe {
            let r = InterCmdRegLow::from_bytes(read_apic(Register::InterCmdRegLow).to_le_bytes());
            return r.delivery_status() == 1;
        }
    }

    unsafe fn send_ipi(&self, low: &InterCmdRegLow, high: &InterCmdRegHigh) {
        write_apic(
            Register::InterCmdRegHigh,
            u32::from_le_bytes(high.into_bytes()),
        );
        write_apic(
            Register::InterCmdRegLow,
            u32::from_le_bytes(low.into_bytes()),
        );

        if self.ipi_pending() {
            panic!("APIC has not completed sending the IPI");
        }
    }

    fn is_supported(&self) -> bool {
        use core::arch::x86_64::__cpuid;
        let feature = unsafe { __cpuid(0x0000_0001) };
        let feature = feature.edx & (1 << 9);
        return feature != 0;
    }

    // IMPORTANT: Fix to be migrated
    unsafe fn init_chained_pics(&self, acpi: &Acpi) {
        PICS.lock().initialize();
        if !acpi.mask_pics {
            // log::info!("Virtual wire mode is active");
            let keyboard_enable = InterruptIndex::Keyboard.as_pic_enable_mask();
            let serial_enable = InterruptIndex::COM1.as_pic_enable_mask()
                & InterruptIndex::COM2.as_pic_enable_mask();
            //TODO: hardcoded should be dynamic throug pci table
            let rtl8139 = InterruptIndex::Rtl8139.as_pic_enable_mask();
            let pic2 = InterruptIndex::Pic2.as_pic_enable_mask();
            // log::info!("rtl8139 mask: {:#x}", rtl8139);
            PICS.lock().mask(keyboard_enable & serial_enable & pic2, rtl8139);
            // PICS.lock().mask(0, 0);
        } else {
            use x86_64::instructions::port::Port;
            let mut imcr_low: Port<u8> = Port::new(0x22);
            let mut imcr_high: Port<u8> = Port::new(0x23);

            imcr_low.write(0x70); // Select imcr register
            imcr_high.write(0x01); // go through apic
            PICS.lock().mask_all();
            log::warn!("Redirecting PIC to io acpi this has not been tested");
        }
    }

    pub unsafe fn init(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        acpi: &Acpi,
    ) {
        if !self.is_supported() {
            panic!("Apic is not available");
        }

        let frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::new(APIC_BASE));
        // Map page for apic base address
        crate::memory::id_map_nocache(mapper, frame_allocator, frame, Some(PageTableFlags::WRITABLE)).unwrap();

        // Enable apic by writing MSR base reg
        let mut apic_base_reg = Msr::new(0x0000_001B);
        let mut base_reg = ApicBaseReg::from_bytes(apic_base_reg.read().to_le_bytes());
        base_reg.set_apic_enable(1);
        base_reg.set_apic_base_addr(0xfee00);
        let payload = u64::from_le_bytes(base_reg.into_bytes());
        apic_base_reg.write(payload);

        self.id = Some(self.apic_id());

        // Only execute if bootstrap core
        if base_reg.bootstrap_core() == 1 {
            self.bsp = Some(true);
            log::info!("BSP is apic id: {}", self.id.unwrap());

            // TODO: Add support for bsp id != 0
            if self.id.unwrap() != 0 {
                panic!("Support for bsp id 0 has to be added");
            }

            // Initialize or mask chained pics
            self.init_chained_pics(acpi);
        }

        // Map spurious interrupts to index
        // and set apic enable bit
        let spur_vec = SpuriousInterReg::new()
            .with_vec(InterruptIndex::Spurious.as_u8())
            .with_apic_enable(1)
            .with_fcc(0);
        let spur_vec = u32::from_le_bytes(spur_vec.into_bytes());
        write_apic(Register::SpurInterVecReg, spur_vec);

        // Read, parse & save apic version
        let apic_version = read_apic(Register::ApicVersion);
        self.version = Some(ApicVersion::from_bytes(apic_version.to_le_bytes()));
        log::info!(
            "APIC version: {}, max lvt entries: {}, extendend: {}",
            self.version.unwrap().ver(),
            self.version.unwrap().max_lvt_entries(),
            self.version.unwrap().extended_apic_space(),
        );

        // Allow all interrupts
        write_apic(Register::TaskPrioReg, 0);

        self.init_timer();
    }

    unsafe fn init_timer(&self) {
        // Divide by two
        let div = DivideConfReg::new().with_div(0).with_div2(0);
        let div = u32::from_le_bytes(div.into_bytes());
        write_apic(Register::DivideConfReg, div);

        // clear mask & periodic timer
        let timer = TimerLvtReg::new()
            .with_vec(InterruptIndex::Timer.as_u8())
            .with_delivery_status(0)
            .with_mask(0)
            .with_timer_mode(1); // Periodic timer inters
        let timer = u32::from_le_bytes(timer.into_bytes());
        write_apic(Register::ApicTimer, timer);

        // Calculate this on every cpu anew
        // by measuring the time with a different clock
        let one_ms = 423845;
        write_apic(Register::TimerInitialCount, one_ms * 1000);
    }

    fn apic_id(&self) -> u8 {
        let id_reg = unsafe { read_apic(Register::ApicId) };
        let res = ApicId::from_bytes(id_reg.to_le_bytes());
        res.aid()
    }
}

unsafe fn read_apic(register: Register) -> u32 {
    let offset = register as u64;
    let ptr = (APIC_BASE + offset) as *mut u32;
    read_volatile(ptr)
}

unsafe fn write_apic(register: Register, value: u32) {
    let offset = register as u64;
    let ptr = (APIC_BASE + offset) as *mut u32;
    write_volatile(ptr, value);
}

pub unsafe fn end_of_interrupt() {
    write_apic(Register::EndOfInterrupt, 0);
}

pub fn msr_is_bsp() -> bool {
    let apic_base_reg = Msr::new(0x0000_001B);
    unsafe {
        let base_reg = ApicBaseReg::from_bytes(apic_base_reg.read().to_le_bytes());
        return base_reg.bootstrap_core() == 1;
    }
}

pub fn local_apic_id() -> u8 {
    use core::arch::x86_64::__cpuid;
    let res = unsafe { __cpuid(0x0000_0001) };

    return (res.ebx >> 24) as u8;
}
