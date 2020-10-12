#![no_std]
#![no_main]

#![feature(custom_test_frameworks)]
#![test_runner(svm_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

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


use svm_kernel::{mylog::LOGGER};

use log::{LevelFilter, info, error};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    // Init & set logger level
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Info);

    #[cfg(test)]
    test_main();
    for i in 0..10{
        info!("Hello World {}", i);
    }

    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!("{}", info);
    loop {}
}


