use lazy_static::lazy_static;
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const PAGE_FAULT_IST_INDEX: u16 = 1;

/*
 * The TSS is an array that holds addresses to different stacks
 * Can be assigned to exception handlers
 */
lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
             // We need the space because else println can
             // overflow the stack. If you enable sse and run
             // into triple faults try increasing this number
            const STACK_SIZE: usize = 4096 * 7;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            // Stacks grow downard thats why the end has to be returned
            stack_end
        };
        tss.interrupt_stack_table[PAGE_FAULT_IST_INDEX as usize] = {
             // We need the space because else println can
             // overflow the stack. If you enable sse and run
             // into triple faults try increasing this number
            const STACK_SIZE: usize = 4096 * 7;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;
            // Stacks grow downard thats why the end has to be returned
            stack_end
        };
        tss
    };
}

use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/*
 * The GDT holds the addresses of the kernel code segment, and
 * the TSS address. The selector registers (cs,ds,...) hold offsets to the GDT
 * to point to the specific entry meant to be used
 */
lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        // Why is a kernel code segment needed?
        // It's needed because it defines the current CPL (privilege level)
        // and some instructions check it by reading the first 3 bits of the cs
        // register. Plus to transition from real mode to long mode the cs register
        // needs to be used I think?
        // Defaults to the Linux Kernel value
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

pub fn init() {
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;
    GDT.0.load();
    unsafe {
        set_cs(GDT.1.code_selector); // Offset to kernel code segment
        load_tss(GDT.1.tss_selector); // Offset to TSS entry
    }
}
