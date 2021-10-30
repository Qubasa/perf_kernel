use x86::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static mut IDT: Option<InterruptDescriptorTable> = None;

pub unsafe fn init() {
    if IDT.is_none() {
        let mut idt = InterruptDescriptorTable::new();
        idt.page_fault.set_handler_fn(page_fault_handler::<14>);
        crate::default_interrupt::init_default_handlers(&mut idt);
        idt.invalid_opcode.set_handler_fn(invalid_op);
        IDT = Some(idt);
    }
    IDT.as_ref().unwrap().load();
}

#[allow(dead_code)]
#[allow(non_camel_case_types)]
#[derive(Debug)]
#[repr(usize)]
enum IndexToException {
    Divide_Error = 0,
    Debug,
    Nmi,
    Breakpoint,
    Overflow,
    Bound_Range_Exceeded,
    Invalid_Opcode,
    Device_Not_Available,
    Double_Fault,
    Coprocessor_Segment_Overrun,
    Invalid_Tss,
    Segment_Not_Present,
    Stack_Segment_Fault,
    General_Protection_Fault,
    Page_Fault = 14,
    reserved0 = 15,
    x87_Floating_Point = 16,
    Alignment_Check,
    Machine_Check,
    Simd_Floating_Point,
    Virtualization = 20,
    reserved1,
    reserved2,
    reserved3,
    reserved4,
    reserved5,
    reserved6,
    reserved7,
    reserved8,
    reserved9,
    Security_Exception = 30,
    Uknown_Software_Defined,
}

impl IndexToException {
    pub fn new(n: usize) -> Self {
        if n > 30 {
            return IndexToException::Uknown_Software_Defined;
        }
        unsafe { core::intrinsics::transmute::<usize, IndexToException>(n) }
    }
}

pub extern "x86-interrupt" fn invalid_op(stack_frame: &mut InterruptStackFrame) {
    log::error!("EXECPTION: Invalid Opcode");
    panic!("{:?}", stack_frame);
}

pub extern "x86-interrupt" fn page_fault_handler<const N: usize>(
    stack_frame: &mut InterruptStackFrame,
    error: PageFaultErrorCode,
) {
    log::error!("EXECPTION: Default Interrupt Handler");
    log::error!(
        "This interrupt has not been initialized: {} page fault error: {:#?}",
        N,
        error
    );
    panic!("{:?}", stack_frame);
}

pub extern "x86-interrupt" fn default_diverging_with_error_handler<const N: usize>(
    stack_frame: &mut InterruptStackFrame,
    error: u32,
) -> ! {
    log::error!("EXECPTION: Default Interrupt Handler");
    log::error!(
        "This interrupt has not been initialized: {} error: {}",
        N,
        error
    );
    log::error!("Exception name: {:#?}", IndexToException::new(N));
    panic!("{:?}", stack_frame);
}

pub extern "x86-interrupt" fn default_diverging_handler<const N: usize>(
    stack_frame: &mut InterruptStackFrame,
) -> ! {
    log::error!("EXECPTION: Default Interrupt Handler");
    log::error!("This interrupt has not been initialized: {}", N);
    log::error!("Exception name: {:#?}", IndexToException::new(N));
    panic!("{:?}", stack_frame);
}

pub extern "x86-interrupt" fn default_handler_with_error<const N: usize>(
    stack_frame: &mut InterruptStackFrame,
    error: u32,
) {
    log::error!("EXECPTION: Default Interrupt Handler");
    log::error!(
        "This interrupt has not been initialized: {}, error code: {}",
        N,
        error
    );

    log::error!("Exception name: {:#?}", IndexToException::new(N));
    panic!("{:?}", stack_frame);
}

pub extern "x86-interrupt" fn default_handler<const N: usize>(
    stack_frame: &mut InterruptStackFrame,
) {
    log::error!("EXECPTION: Default Interrupt Handler");
    log::error!("This interrupt has not been initialized: {}", N);

    log::error!("Exception name: {:#?}", IndexToException::new(N));

    panic!("{:?}", stack_frame);
}
