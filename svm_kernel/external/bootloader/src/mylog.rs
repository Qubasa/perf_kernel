use log::{Level, Metadata, Record};

use crate::println;
use crate::serial::SERIAL_WRITER;
use crate::vga::VGA_WRITER;

pub struct HWLogger;

pub static LOGGER: HWLogger = HWLogger;

impl log::Log for HWLogger {
    // Enable logging at level Trace if cond is met
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    // Executed on log macros
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        unsafe {
            SERIAL_WRITER.as_mut().unwrap().send(0xC); // TODO: Does not clear screen
            VGA_WRITER.as_mut().unwrap().flush();
        }
    }
}
