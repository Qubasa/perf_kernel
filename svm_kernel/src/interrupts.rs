use crate::apic;
use crate::gdt;
use crate::print;

use pic8259_simple::ChainedPics;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

// Offset the PICs to avoid index collision with
// exceptions in the IDT
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// IDT index numbers
// IMPORTANT: Fix to be migrated
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    LegacyTimer = PIC_1_OFFSET,
    Keyboard, // 33
    Pic2,     // Secondary interrupt controller
    COM2,
    COM1,
    IRQ5, // 37
    FloppyController,
    MasterPicSpurious, // or parallel port interrupt

    // Pic2
    RtcTimer,
    ACPI,
    ScsiNic1,
    Rtl8139,
    Mouse,
    MathCoProcessor,
    AtaChannel1,
    AtaChannel2,

    // ??
    IRQ16,
    SlavePicSpurious,
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
    //IMPORTAT: Fix to be migrated
    pub fn as_pic_enable_mask(self) -> u8 {
        let mut diff = self.as_usize() - InterruptIndex::LegacyTimer.as_usize();
        if diff > 7 {
            diff -= 8;
        }
        let mask = 0xff & !(1 << diff);
        mask as u8
    }
}
pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });
pub static APIC: spin::Mutex<apic::Apic> = spin::Mutex::new(apic::Apic::new());

// Global static IDT
lazy_static::lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.simd_floating_point.set_handler_fn(simd_floatingpoint_handler);
        idt.security_exception.set_handler_fn(security_handler);
        idt.virtualization.set_handler_fn(virtualization_handler);
        idt.machine_check.set_handler_fn(machine_check_handler);
        idt.alignment_check.set_handler_fn(alignment_handler);
        idt.x87_floating_point.set_handler_fn(x87_floatingpoint_handler);
        unsafe {
            idt.page_fault.set_handler_fn(page_fault_handler)
            // Use a different stack in case of kernel stack overflow
            .set_stack_index(gdt::PAGE_FAULT_IST_INDEX)
        };
        idt.general_protection_fault.set_handler_fn(general_prot_handler);
        idt.stack_segment_fault.set_handler_fn(stack_segment_handler);
        idt.segment_not_present.set_handler_fn(segment_not_present_handler);
        idt.invalid_tss.set_handler_fn(invalid_tss_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
            // Use a different stack in case of kernel stack overflow
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX)
        };
        idt.device_not_available.set_handler_fn(device_not_available_handler);
        idt.invalid_opcode.set_handler_fn(invalid_op_handler);
        idt.bound_range_exceeded.set_handler_fn(bound_range_handler);
        idt.overflow.set_handler_fn(overflow_handler);
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.non_maskable_interrupt.set_handler_fn(non_maskable_handler);
        idt.debug.set_handler_fn(debug_handler);
        idt.divide_error.set_handler_fn(divide_error_handler);

        crate::default_interrupt::init_default_handlers(&mut idt);

        // User defined
        idt[InterruptIndex::Timer.as_usize()]
            .set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()]
            .set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::COM2.as_usize()]
            .set_handler_fn(serial_handler);
        idt[InterruptIndex::COM1.as_usize()]
            .set_handler_fn(serial_handler);
        idt[InterruptIndex::Spurious.as_usize()]
            .set_handler_fn(spurious_handler);
        idt[InterruptIndex::SlavePicSpurious.as_usize()]
            .set_handler_fn(spurious_handler);
        idt[InterruptIndex::MasterPicSpurious.as_usize()]
            .set_handler_fn(spurious_handler);
        idt
    };
}

pub fn load_idt() {
    IDT.load();
}

use crate::hlt_loop;
use x86_64::structures::idt::PageFaultErrorCode;

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    log::error!("EXCEPTION: PAGE FAULT");
    log::error!("Accessed Address: {:?}", Cr2::read());
    log::error!("Error Code: {:?}", error_code);
    log::error!("{:#?}", stack_frame);
    hlt_loop();
}

pub extern "x86-interrupt" fn default_handler<const N: usize>(
    stack_frame: InterruptStackFrame,
) {
    log::error!("EXECPTION: Default Interrupt Handler");
    log::error!("This interrupt has not been initialized: {}", N);
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn general_prot_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    log::error!("EXCEPTION: General Protection Exception");
    log::error!("Error Code: {:?}", error_code);
    log::error!("{:#?}", stack_frame);
    hlt_loop();
}

// TODO: Enable alignment checking
extern "x86-interrupt" fn alignment_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    log::error!("EXCEPTION: Alignment Exception");
    log::error!("Error Code: {:?}", error_code);
    log::error!("{:#?}", stack_frame);
    hlt_loop();
}

// Keyboard handler
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    // Initialize pc_keyboard crate
    // This will only be called on kernel start don't worry
    lazy_static::lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = {
            // Use US layout & ignore ctrl+key characters
            Mutex::new(
                Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
            )
        };
    }

    // Lock keyboard parser
    let mut keyboard = KEYBOARD.lock();
    // Create port on mem addr 0x60 to read keycode
    let mut port = Port::new(0x60);
    // Read keycode from mem
    let scancode: u8 = unsafe { port.read() };

    // Parse keycode with pc_keyboard crate
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:#?}", key),
            }
        }
    }

    // Renable interrupts again
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

// Breakpoint handler
extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    log::error!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

// Double fault handler
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn invalid_op_handler(stack_frame: InterruptStackFrame) {
    panic!("INVALID OP HANDLER\n{:#?}", stack_frame);
}

// Serial handler
extern "x86-interrupt" fn serial_handler(_stack_frame: InterruptStackFrame) {
    // Disable interrupts because we lock the SERIAL_WRITER here
    x86_64::instructions::interrupts::without_interrupts(|| {
        let data = [crate::serial::SERIAL_WRITER.lock().read(); 1];
        print!("{}", alloc::str::from_utf8(&data).unwrap());
    });

    // Renable interrupts again
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::COM2.as_u8());
    }
}

// timer interrupt handler
extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    print!(".");

    // Renable interrupts again
    unsafe {
        apic::end_of_interrupt();
    }

}

extern "x86-interrupt" fn spurious_handler(_stack_frame: InterruptStackFrame) {
    log::info!("SPURIOUS HANDLER");

    // Check if this is a pic8259_simple spurious interrupt or a legitimate interrupt
    unsafe {
        if PICS.lock().is_spurious_interrupt(|| {
            log::info!("This is a pic8259_simple spurious interrupt");
        }) {
            return;
        } else {
            panic!("Not a spurious interrupt, that's bad :o");
        }
    }
}

/*
 *
 * Non populated cpu exceptions
 *
 */
extern "x86-interrupt" fn debug_handler(stack_frame: InterruptStackFrame) {
    log::info!("debug exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn divide_error_handler(stack_frame: InterruptStackFrame) {
    log::info!("divide error exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn non_maskable_handler(stack_frame: InterruptStackFrame) {
    log::info!("non maskable interrupt exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn overflow_handler(stack_frame: InterruptStackFrame) {
    log::error!("overflow exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn bound_range_handler(stack_frame: InterruptStackFrame) {
    log::error!("bound range exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn device_not_available_handler(stack_frame: InterruptStackFrame) {
    log::error!("device not available exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn invalid_tss_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    log::error!("invalid tss exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn segment_not_present_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    log::error!("segment not present exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn stack_segment_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    log::error!("stack segment fault exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn x87_floatingpoint_handler(stack_frame: InterruptStackFrame) {
    log::error!("x87_floatingpoint exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn machine_check_handler(stack_frame: InterruptStackFrame) -> ! {
    log::error!("Machine check exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn simd_floatingpoint_handler(stack_frame: InterruptStackFrame) {
    log::error!("Simd floatingpoint exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn virtualization_handler(stack_frame: InterruptStackFrame) {
    log::error!("virtualization exception");
    panic!("{:?}", stack_frame);
}

extern "x86-interrupt" fn security_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    log::error!("security exception");
    panic!("{:?}", stack_frame);
}

// Executed on cargo test
