#![no_std]
#![no_main]

use bootloader::*;

extern "C" {
    static mmap_ent: usize;
    static _memory_map: usize;
    static _kernel_start_addr: usize;
    static _kernel_end_addr: usize;
    static _kernel_size: usize;
    static __page_table_start: usize;
    static __page_table_end: usize;
    static __bootloader_end: usize;
    static __bootloader_start: usize;
    static _p4: usize;
}

#[no_mangle]
fn bootloader_main() {
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
    }
}
