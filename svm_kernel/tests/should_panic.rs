#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]


use core::panic::PanicInfo;
use svm_kernel::{QemuExitCode, exit_qemu, println, init};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    svm_kernel::hlt_loop();
}
#[no_mangle]
pub extern "C" fn _start() -> ! {
    init();
    test_main();

    svm_kernel::hlt_loop();
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("===== should_panic test =====");
    for test in tests {
        test();
        println!("[test did not panic]");
        exit_qemu(QemuExitCode::Failed);
    }
    exit_qemu(QemuExitCode::Success);
}

#[test_case]
fn should_fail() {
    assert_eq!(0, 1);
}
