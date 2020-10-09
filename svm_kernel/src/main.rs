#![no_std]
#![no_main]

/*
 * Followed the tutorial here: https://os.phil-opp.com
 * TODO: Replace builtin memcpy, memset with optimized one
 */

/*
 * This kernel has been tested on an AMD x64 processor
 * family: 0x17h, model: 0x18h
 */

mod print;
mod serial;
mod vga;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    for i in 0..50{
        println!("Hello World {}", i);
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
