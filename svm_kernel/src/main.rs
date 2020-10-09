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

use core::panic::PanicInfo;
use uart_16550::SerialPort;

static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let vga_buffer = 0xb8000 as *mut u8;

    let mut serial_port = unsafe { SerialPort::new(0x3F8) };
    serial_port.init();

    for(i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2+1) = 0xb;
        }
        serial_port.send(byte);
    };

    // let serial_port_base = 0x3f8 as *mut u8; // Also COM1

    // // Configure
    // unsafe {
    //     /*
    //      *  Configures the line of the given serial port. The port is set to have a
    //      *  data length of 8 bits, no parity bits, one stop bit and break control
    //      *  disabled.
    //      */
    //     *serial_port_base.offset(3) = 0x03;
    //     *serial_port_base.offset(2) = 0xC7; // Enable fifo, 14b queue size, clear queues
    //     *serial_port_base.offset(4) = 0x03; // RTS = 1, DTS = 1

    // }


    // for(_i, &byte) in HELLO.iter().enumerate() {
    //     unsafe {
    //         while(*serial_port_base.offset(5) & 0x20 == 0){};
    //         *serial_port_base = byte;
    //     }
    // }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {};
}


