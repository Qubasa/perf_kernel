
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// Global static IDT
lazy_static::lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

// Breakpoint hanlder
extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame){
    log::error!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}


// Executed on cargo test
#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}
