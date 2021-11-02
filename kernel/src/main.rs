#![no_std]
#![no_main]
#![feature(custom_test_frameworks)] // https://github.com/rust-lang/rfcs/blob/master/text/2318-custom-test-frameworks.md
#![test_runner(svm_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(asm)]
#![feature(test)]
#![feature(bench_black_box)]
#![allow(unreachable_code)]
/*
 * Followed the tutorial here: https://os.phil-opp.com
 * TODO: Replace builtin memcpy, memset with optimized one
 */

/* TODO:
 * Write bootloader myself to be able to enable
 * mmx,sse & float features!
 * Should also solve the lto linktime warning
 */

/*
 * This kernel has been tested on an AMD x64 processor
 * family: 0x17h, model: 0x18h
 */

use bootloader::bootinfo;
use bootloader::entry_point;
extern crate alloc;

/*
 * KERNEL MAIN
 * The macro entry_point creates the nomangle _start func for us and checks that
 * the given function has the correct signature
 */
//TODO: rsp has to be 16 byte aligned
entry_point!(kernel_main);
fn kernel_main(_boot_info: &'static bootinfo::BootInfo) -> ! {
    unsafe {
        // Initialize routine for kernel
        svm_kernel::init(_boot_info);
    };

    // This func gets generated by cargo test
    #[cfg(test)]
    test_main();

    svm_kernel::hlt_loop();
}

/*
 * KERNEL PANIC HANDLER
 * Not used in cargo test
 */
//TODO: Implement a bare metal debugger
// https://lib.rs/crates/gdbstub
// https://sourceware.org/gdb/onlinedocs/gdb/Remote-Protocol.html
// TODO: Make panic handler print stuff without a global lock
// If an error occurs while reading memory inside the print lock
// a deadlock occurs
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    svm_kernel::println!("{}", info);

    #[cfg(debug)]
    svm_kernel::exit_qemu(svm_kernel::QemuExitCode::Failed);

    #[cfg(not(debug))]
    loop {}
}