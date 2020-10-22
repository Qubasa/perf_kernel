#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use svm_kernel::{exit_qemu, init, println, QemuExitCode};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("{}", _info);
    exit_qemu(QemuExitCode::Success);
    loop {}
}
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running stack overflow test");
    for test in tests {
        test();
        println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    let x = 0;
    stack_overflow();
    unsafe {
        core::ptr::read_volatile(&x);
    }
}

#[test_case]
fn stack_overflow_test() {
    stack_overflow();
    log::error!("Execution continued after double fault!");
    exit_qemu(QemuExitCode::Failed);
}
