#![no_std]
#![no_main]

use bootloader::*;

#[no_mangle]
fn stage_4() {

}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
    }
}
