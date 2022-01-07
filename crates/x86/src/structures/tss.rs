//! Provides a type for the task state segment structure.

use crate::registers::eflags::EFlags;
use crate::VirtAddr;


/// Stack pointer substruct with padding used in the TSS
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct StackPointers {
    /// ESPn
    pub stack_segment_offset: VirtAddr,
    /// SSn
    pub stack_segment_selector: u16,
    reserved: u16,
}

impl StackPointers {
    /// Initialize offset and selector to zero
    pub const fn zero() -> Self {
        Self {
            stack_segment_offset: VirtAddr::zero(),
            stack_segment_selector: 0,
            reserved: 0,
        }
    }
}

/// In 64-bit mode the TSS holds information that is not
/// directly related to the task-switch mechanism,
/// but is used for finding kernel level stack
/// if interrupts arrive while in kernel mode.
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct TaskStateSegment {
    /// Contains a copy of a task selector from previously executed task
    pub link: u16,
    reserved_1: u16,
    /// Contains the privilege 0,1,2 stack pointers for task
    pub stack_pointer: [StackPointers; 3],
    /// Contains the page translation table base address
    pub cr3: u32,
    /// Contains the Instruction Pointer
    pub eip: VirtAddr,
    /// Contains a copy of the EFLAGS image at the point the task is suspended
    pub eflags: EFlags,
    /// General purpose register
    pub eax: u32,
    /// General purpose register
    pub ecx: u32,
    /// General purpose register
    pub edx: u32,
    /// General purpose register
    pub ebx: u32,
    /// General purpose register
    pub esp: u32,
    /// General purpose register
    pub ebp: u32,
    /// General purpose register
    pub esi: u32,
    /// General purpose register
    pub edi: u32,
    /// Segment selector registers
    pub es: u16,
    reserved2: u16,
    /// Segment selector registers
    pub cs: u16,
    reserved3: u16,
    /// Segment selector registers
    pub ss: u16,
    reserved4: u16,
    /// Segment selector registers
    pub ds: u16,
    reserved5: u16,
    /// Segment selector registers
    pub fs: u16,
    reserved6: u16,
    /// Segment selector registers
    pub gs: u16,
    reserved7: u16,
    /// Contains the local descriptor table segment selector for the task
    pub ldt_selector: u16,
    reserved8: u32,

    /// The 16-bit offset to the I/O permission bit map from the 64-bit TSS base.
    pub iomap_base: u16,
}

impl TaskStateSegment {
    /// Creates a new TSS with zeroed privilege and interrupt stack table and a zero
    /// `iomap_base`.
    #[inline]
    pub const fn new() -> TaskStateSegment {
        TaskStateSegment {
            link: 0,
            reserved_1: 0,
            stack_pointer: [StackPointers::zero(); 3],
            cr3: 0,
            eip: VirtAddr::zero(),
            eflags: EFlags::empty(),
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            esp: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            es: 0,
            reserved2: 0,
            cs: 0,
            reserved3: 0,
            ss: 0,
            reserved4: 0,
            ds: 0,
            reserved5: 0,
            fs: 0,
            reserved6: 0,
            gs: 0,
            reserved7: 0,
            ldt_selector: 0,
            reserved8: 0,
            iomap_base: 0,
        }
    }
}
