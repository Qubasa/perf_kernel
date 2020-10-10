use log::{Record, Level, Metadata};

use crate::serial::SERIAL_WRITER;
use crate::vga::VGA_WRITER;
use crate::println;

pub struct HWLogger;

pub static LOGGER: HWLogger = HWLogger;

impl log::Log for HWLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {
        SERIAL_WRITER.lock().send(0xC); // TODO: Does not clear screen
        VGA_WRITER.lock().flush();
    }
}

