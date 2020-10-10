#![no_std]
#![no_main]

/*
 * Followed the tutorial here: https://os.phil-opp.com
 * TODO: Replace builtin memcpy, memset with optimized one
 */

/* TODO:
 * Write bootloader myself to be able to enable
 * mmx,sse & float features!
 * Should also solve the lto linktime error
 */

/*
 * This kernel has been tested on an AMD x64 processor
 * family: 0x17h, model: 0x18h
 */

mod print;
mod serial;
mod vga;
mod mylog;

use log::{LevelFilter, info, Log};

#[no_mangle]
pub extern "C" fn _start() -> ! {

    // Init & set logger level
    log::set_logger(&mylog::LOGGER).unwrap();
    log::set_max_level(LevelFilter::Info);


    for i in 0..10{
        info!("Hello World {}", i);
    }

    mylog::LOGGER.flush();

    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
