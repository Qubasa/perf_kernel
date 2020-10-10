
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::vga::_print(format_args!($($arg)*));
        $crate::serial::_print(format_args!($($arg)*))
    };
}

#[test_case]
fn test_println_many() {
    for i in 0..60 {
        println!("test_println_many output: {}", i);
    }
}
