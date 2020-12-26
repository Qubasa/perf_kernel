#![no_std]
#![no_main]

use bootloader::*;

#[no_mangle]
fn bootloader_main() {
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
    }
}
