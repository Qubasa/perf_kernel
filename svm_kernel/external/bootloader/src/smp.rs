use core::arch::x86::{__cpuid};

pub fn num_cores() -> u32 {
    unsafe {
        let res = __cpuid(0x8000_0008);
        return (res.ecx & 0xFF) + 1;
    };
}

pub fn apic_id() -> u8 {
    unsafe {
        let res = __cpuid(0x8000_0008);
        return (res.ebx & (0xFF << 24)) as u8;
    };
}


#[no_mangle]
unsafe extern "C" fn smp_main() {
    // exit_qemu(QemuExitCode::Failed);
    // log::info!("=== smp main! === ");
    // crate::vga::_print(format_args!("Hello World"));
    loop {};
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}


pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }

    panic!("Failed to exit Qemu");
}
