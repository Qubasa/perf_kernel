#![allow(dead_code)]

use crate::acpi::Acpi;
use crate::interrupts::InterruptIndex;
use crate::interrupts::PICS;
use core::ptr::{read_volatile, write_volatile};
use modular_bitfield::prelude::*;
use x86_64::registers::model_specific::Msr;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, Size4KiB};

// Other constants
const APIC_BASE: u64 = 0x0_0000_FEE0_0000;

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
    InterCmdRegLow = 0x300,
    InterCmdRegHigh = 0x310,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct InterCmdRegLow {
    vec: B8,
    msg_type: B3,
    dest_mode: B1,
    delivery_status: B1,
    res0: B1,
    level: B1,
    trigger_mode: B1,
    remote_read_status: B2,
    dest_shorthand: B2,
    res1: B12,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
struct InterCmdRegHigh {
    res1: B24,
    dest: B8,
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
    version: Option<ApicVersion>,
}

impl Apic {
    pub const fn new() -> Self {
        Apic {
            apic_base_reg: Msr::new(0x0000_001B),
            version: None,
        }
    }

    pub unsafe fn mp_init(&self, apic_id: u8, trampoline: unsafe extern "C" fn() -> !) {

        // Send INIT ipi
        let low = InterCmdRegLow::new()
            .with_vec(0) // INIT needs vec to be zero
            .with_trigger_mode(0) // level-sensitive
            .with_msg_type(0b101) // INIT type
            .with_level(0) // 0 for INIT
            ;
        let high = InterCmdRegHigh::new().with_dest(apic_id);
        self.send_ipi(&low, &high);

        log::info!("trampoline addr: {:?}", trampoline);
        let to_vec = trampoline as u64 >> 12;
        log::info!("vec addr: {}", to_vec);
        // // Send STARTUP ipi
        let low = InterCmdRegLow::new()
            .with_vec(to_vec as u8) // Core execute code at 0x000VV000
            .with_trigger_mode(0) // level-sensitive
            .with_msg_type(0b110) // STARTUP type
            .with_level(1) // 1 for everything else
            ;
        let high = InterCmdRegHigh::new().with_dest(apic_id);
        self.send_ipi(&low, &high);
        self.send_ipi(&low, &high);
    }

    fn ipi_pending(&self) -> bool {
        unsafe {
        let r = InterCmdRegLow::from_bytes(self.read_apic(Register::InterCmdRegLow).to_le_bytes());
        return r.delivery_status() == 1;
        }
    }

    unsafe fn send_ipi(&self, low: &InterCmdRegLow, high: &InterCmdRegHigh) {
        self.write_apic(
            Register::InterCmdRegHigh,
            u32::from_le_bytes(high.into_bytes()),
        );
        self.write_apic(
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

    unsafe fn init_chained_pics(&self, acpi: &Acpi) {
        PICS.lock().initialize();
        if !acpi.mask_pics {
            log::info!("Virtual wire mode is active");
            let keyboard_enable = InterruptIndex::Keyboard.as_pic_enable_mask();
            let serial_enable = InterruptIndex::COM1.as_pic_enable_mask()
                & InterruptIndex::COM2.as_pic_enable_mask();
            PICS.lock().mask(keyboard_enable & serial_enable, 0xff);
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

    pub unsafe fn initialize(
        &mut self,
        mapper: &mut OffsetPageTable,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
        acpi: &Acpi,
    ) {
        if !self.is_supported() {
            panic!("Apic is not available");
        }

        let choco_low = InterCmdRegLow::from_bytes(0x4500u32.to_le_bytes());
        log::info!("1: {:?}", choco_low);
        let choco_low = InterCmdRegLow::from_bytes(0x4608u32.to_le_bytes());
        log::info!("1: {:?}", choco_low);

        // Initialize or mask chained pics
        self.init_chained_pics(acpi);

        // Map page for apic base address
        crate::memory::id_map_nocache(mapper, frame_allocator, x86_64::PhysAddr::new(APIC_BASE))
            .unwrap();

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
