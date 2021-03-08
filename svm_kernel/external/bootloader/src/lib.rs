//! This library part of the bootloader allows kernels to retrieve information from the bootloader.
//!
//! To combine your kernel with the bootloader crate you need a tool such
//! as [`bootimage`](https://github.com/rust-osdev/bootimage). See the
//! [_Writing an OS in Rust_](https://os.phil-opp.com/minimal-rust-kernel/#creating-a-bootimage)
//! blog for an explanation.

#![no_std]
#![feature(global_asm)]
#![feature(llvm_asm)]
#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

// The dependencies here are set to target_arch = x86 because
// the 'bootimage' command first builds this crate as dependencie of the kernel
// which is in x86_64 build mode. As some dependencies do not build in x86_64
// we create an "empty" crate for x86_64 and if bootimage then directly builds
// this crate in x86 mode everything will be build accordingly.

pub mod bootinfo;

#[cfg(target_arch="x86")]
pub mod serial;
#[cfg(target_arch="x86")]
pub mod vga;
#[cfg(target_arch="x86")]
pub mod print;
#[cfg(target_arch="x86")]
pub mod mylog;


global_asm!(include_str!("boot.s"));




/// Defines the entry point function.
///
/// The function must have the signature `fn(&'static BootInfo) -> !`.
///
/// This macro just creates a function named `_start`, which the linker will use as the entry
/// point. The advantage of using this macro instead of providing an own `_start` function is
/// that the macro ensures that the function and argument types are correct.
#[macro_export]
macro_rules! entry_point {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "C" fn __impl_start(boot_info: &'static $crate::bootinfo::BootInfo) -> ! {
            // validate the signature of the program entry point
            let f: fn(&'static $crate::bootinfo::BootInfo) -> ! = $path;

            f(boot_info)
        }
    };
}

