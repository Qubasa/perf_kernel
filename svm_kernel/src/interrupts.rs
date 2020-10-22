use crate::gdt;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// Offset the PICs to avoid index collision with
// exceptions in the IDT
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

use pic8259_simple::ChainedPics;
use crate::{println, print};

// Initialize the Process Interrupt Controller
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard // 33
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

// Global static IDT
lazy_static::lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
            // Use a different stack in case of kernel stack overflow
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt[InterruptIndex::Timer.as_usize()]
                .set_handler_fn(timer_interrupt_handler);
            idt[InterruptIndex::Keyboard.as_usize()]
                .set_handler_fn(keyboard_interrupt_handler);
        }
        idt
    };
}


pub fn init_idt() {
    IDT.load();
}

// Keyboard handler
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {

    use x86_64::instructions::port::Port;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read()  };
    print!("{}", scancode);

    // Renable interrupts again
    unsafe {
        PICS.lock().notify_end_of_interrupt(
            InterruptIndex::Keyboard.as_u8()
        );
    }
}

// Breakpoint handler
extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    log::error!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

// Double fault handler
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}


// timer interrupt handler
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: &mut InterruptStackFrame)
{
    print!(".");

    // Renable interrupts again
    unsafe {
        PICS.lock().notify_end_of_interrupt(
            InterruptIndex::Timer.as_u8()
        );
    }
}

// Executed on cargo test
#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
}
