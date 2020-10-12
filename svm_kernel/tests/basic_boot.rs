#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(svm_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use svm_kernel::{println, mylog::LOGGER};

use log::{LevelFilter, info, error};
#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Trace);
    test_main();

    loop {}
}

#[test_case]
fn test_println() {
    println!("test_println output");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    svm_kernel::test_panic_handler(info)
}

#[test_case]
fn test_log() {
    use log::{error, warn, info, debug, trace};
    trace!("trace log");
    println!("LOG LEVEL: {}", log::max_level());
    error!("error log");
    warn!("warn log");
    info!("info log");
    debug!("debug log");
    trace!("trace log");
}
