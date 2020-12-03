use modular_bitfield::prelude::*;

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
pub struct ApicId {
    pub res0: B24,
    pub aid: B8
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct InterCmdRegLow {
    pub vec: B8,
    pub msg_type: B3,
    pub dest_mode: B1,
    pub delivery_status: B1,
    pub res0: B1,
    pub level: B1,
    pub trigger_mode: B1,
    pub remote_read_status: B2,
    pub dest_shorthand: B2,
    pub res1: B12,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct InterCmdRegHigh {
    pub res1: B24,
    pub dest: B8,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct DestFormatReg {
    pub res: B28,
    pub model: B4,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct LogicalDestReg {
    pub res: B24,
    pub dest_logical_id: B8,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct TaskPrioReg {
    pub task_prio: B4,
    pub task_prio_subclass: B4,
    pub res0: B24,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct ApicVersion {
    pub ver: B8,
    pub res0: B8,
    pub max_lvt_entries: B8,
    pub res1: B7,
    pub extended_apic_space: B1,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct ApicBaseReg {
    pub res0: B8,
    pub bootstrap_core: B1,
    pub res1: B2,
    pub apic_enable: B1,
    pub apic_base_addr: B40,
    pub res2: B12,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct DivideConfReg {
    pub div: B2,
    pub rev0: B1,
    pub div2: B1,
    pub res1: B28,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct TimerLvtReg {
    pub vec: B8,
    pub res0: B4,
    pub delivery_status: B1,
    pub res1: B3,
    pub mask: B1,
    pub timer_mode: B1,
    pub res2: B14,
}

#[bitfield]
#[derive(Debug, Clone, Copy)]
pub struct SpuriousInterReg {
    pub vec: B8,
    pub apic_enable: B1,
    pub fcc: B1,
    pub res0: B22,
}

