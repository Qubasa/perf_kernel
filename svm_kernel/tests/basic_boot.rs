#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(svm_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use log::{debug, error, info, trace, warn};
use svm_kernel::{mylog::LOGGER, println};

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    println!("==== test_logging ====");
    test_main();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    svm_kernel::test_panic_handler(info)
}

#[test_case]
fn test_log() {
    trace!("trace log");
    println!("LOG LEVEL: {}", log::max_level());
    error!("error log");
    warn!("warn log");
    info!("info log");
    debug!("debug log");
    trace!("trace log");
}
