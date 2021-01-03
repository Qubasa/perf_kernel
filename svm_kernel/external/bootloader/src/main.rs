#![no_std]
#![no_main]
#![feature(global_asm)]

#[allow(unused_imports)]
use bootloader::*;


global_asm!(include_str!("boot.s"));

extern "C" {
    static _kernel_start_addr: usize;
}

#[no_mangle]
unsafe fn bootloader_main() {
    let ptr = 0xb8000 as *mut u32;
    *ptr = 0x2f4b2f4f + _kernel_start_addr as u32;
    loop {}
}


#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
    }
}
