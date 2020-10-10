use uart_16550::SerialPort;

lazy_static::lazy_static! {
    // Rust ref keyword explained
    // http://xion.io/post/code/rust-patterns-ref.html
    pub static ref SERIAL_WRITER: spin::Mutex<SerialPort> = spin::Mutex::new(
        unsafe {
            let mut serial_port = SerialPort::new(0x3F8);
            serial_port.init();
            serial_port
         }
    );
}

use core::fmt;
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    SERIAL_WRITER.lock().write_fmt(args).unwrap();
}
