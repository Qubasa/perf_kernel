#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        {
        $crate::serial::_print(format_args!($($arg)*));
        $crate::vga::_print(format_args!($($arg)*));
        }
    };
}
