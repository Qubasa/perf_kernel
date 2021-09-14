#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(test_runner)]
#![reexport_test_harness_main = "test_main"]

use bootloader::{entry_point, bootinfo::BootInfo};
use core::panic::PanicInfo;
use svm_kernel::{exit_qemu, init, println, QemuExitCode};

#[allow(unreachable_code)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    exit_qemu(QemuExitCode::Success);
    loop {}
}
entry_point!(main);

fn main(boot_info: &'static BootInfo) -> ! {
    init(boot_info);
    test_main();

    loop {}
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    println!("==== stack overflow test ====");
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
