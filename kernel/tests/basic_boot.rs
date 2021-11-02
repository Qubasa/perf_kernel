#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(perf_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use log::{debug, error, info, trace, warn};
use perf_kernel::{klog, println};

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    klog::init();
    log::set_max_level(log::LevelFilter::Trace);
    println!("==== test_logging ====");
    test_main();

    perf_kernel::hlt_loop();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    perf_kernel::test_panic_handler(info)
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
