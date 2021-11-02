use log::{Level, Metadata, Record};

use crate::println;
use crate::serial::SERIAL_WRITER;
use crate::vga::VGA_WRITER;

pub struct HWLogger;

static mut LOGGER: Option<HWLogger> = None;

pub fn init() {
    unsafe {
        if LOGGER.is_none() {
            crate::serial::init();
            crate::vga::init();

            LOGGER = Some(HWLogger);
            // Init & set logger level
            log::set_logger(LOGGER.as_ref().unwrap()).unwrap();
        }

        log::set_max_level(log::LevelFilter::Info);
    }
}

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
            SERIAL_WRITER.as_ref().unwrap().lock().send(0xC); // TODO: Does not clear screen
            VGA_WRITER.as_ref().unwrap().lock().flush();
        };
    }
}
