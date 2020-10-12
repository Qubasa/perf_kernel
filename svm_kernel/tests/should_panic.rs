#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]


use core::panic::PanicInfo;
use svm_kernel::{QemuExitCode, exit_qemu, println};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}
#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test();
        println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn should_fail() {
    println!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}
