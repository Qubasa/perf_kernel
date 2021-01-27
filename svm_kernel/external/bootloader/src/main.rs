#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(asm)]

use multiboot2;
use bootloader::mylog::LOGGER;
use log::LevelFilter;

global_asm!(include_str!("boot.s"));
global_asm!(include_str!("start.s"));
extern "C" {
    static _kernel_start_addr: usize;
}

#[no_mangle]
extern "C" fn bootloader_main(magic: u32, mboot2_info_ptr: u32) {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(LevelFilter::Info);
    log::info!("Bootloader main.");

    if magic != 0x36d76289 {
        panic!(
            "EAX magic is incorrect. Booted from a non compliant bootloader: {:#x}",
            magic
        );
    }

    log::info!("Parsing multiboot headers at addr: {:#x}", mboot2_info_ptr);
    let boot_info = unsafe { multiboot2::load(mboot2_info_ptr as usize) };

    log::info!("Boot info: {:?}", boot_info);

    loop {}
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use bootloader::println;
    println!("ERROR: {}", info);
    loop {}
}
