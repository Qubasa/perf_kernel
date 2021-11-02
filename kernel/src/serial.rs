use uart_16550::SerialPort;

// Serial programming resource:
// https://en.wikibooks.org/wiki/Serial_Programming/8250_UART_Programming

pub static mut SERIAL_WRITER: Option<spin::Mutex<SerialPort>> = None;

pub unsafe fn init() {
    let mut serial_port = SerialPort::new(0x3F8);
    serial_port.init();
    SERIAL_WRITER = Some(spin::Mutex::new(serial_port));
}

use core::fmt;
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| unsafe {
        SERIAL_WRITER
            .as_ref()
            .unwrap()
            .lock()
            .write_fmt(args)
            .unwrap();
    });
}
