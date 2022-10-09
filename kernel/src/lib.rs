#![feature(result_contains_err)]
#![feature(stmt_expr_attributes)]
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]
#![feature(bench_black_box)]
#![feature(const_mut_refs)]
#![feature(test)]
#![feature(maybe_uninit_uninit_array)]
#![no_std]
#![allow(clippy::missing_safety_doc)]

pub mod acpi;
pub mod acpi_regs;
pub mod allocator;
pub mod apic;
pub mod apic_regs;
pub mod bench;
pub mod corestate;
pub mod default_interrupt;
pub mod interrupts;
pub mod klog;
pub mod memory;
pub mod pci;
pub mod print;
pub mod serial;
pub mod smp;
pub mod time;
pub mod tss;
pub mod vga;

extern crate alloc;

/*
 * Use an exit code different from 0 and 1 to
 * differentiate between qemu error or kernel quit
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

// Write to port 0xf4 to exit qemu
pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }

    panic!("Failed to exit Qemu");
}

// All kernel inits summed up
pub unsafe fn init(boot_info: &'static bootloader::bootinfo::BootInfo) {
    klog::init();

    // Make sure that other cores have the same register state like bsp
    // if apic::is_bsp() {
    //     corestate::save_corestate();
    // } else {
    //     //smp::check_corestate();
    // }

    // Init online status of cores
    smp::init();

    log::debug!("bootinfo: {:#x?}", boot_info.memory_map);

    // Load gdt into current cpu with lgdt
    // Also set code and tss segment selector registers
    tss::init(boot_info);

    // Load idt into the current cpu with lidt
    interrupts::init();

    // Create OffsetPageTable instance by
    // calculating address with: Cr3::read() + offset from bootloader
    let (mapper, frame_allocator) = memory::init(boot_info);

    if apic::is_bsp() {
        // Measure speed of rtsc once
        time::calibrate();

        // Check support of hardware features needed for benchmarking
        bench::check_support();

        // Initialize the heap allocator
        // by mapping the heap pages
        allocator::init_heap(
            mapper.lock().deref_mut(),
            frame_allocator.lock().deref_mut(),
        )
        .expect("heap init failed");
    }

    log::debug!("Init apic controller");

    // Parse acpi tables once
    let acpi = acpi::init();

    // Initialize lapic controller
    apic::init(
        mapper.lock().deref_mut(),
        frame_allocator.lock().deref_mut(),
        acpi,
        boot_info,
    );

    {
        let (core, core_index) = boot_info
            .cores
            .get_by_apic_id(crate::apic::apic_id())
            .unwrap();

        log::info!(
            "Enabling interrupts for core index {} apic_id {}",
            core_index,
            core.get_apic_id().unwrap()
        );
    }
    // Enable interrupts
    x86_64::instructions::interrupts::enable();

    if apic::is_bsp() {
        for lapic in acpi.apics.as_ref().unwrap().iter().skip(1) {
            apic::mp_init(lapic.id, boot_info.smp_trampoline);
            //time::sleep(100);
        }
    }

    // Search for pci devices
    //pci::init();
    // Init pci devices
    //TODO: uncomment
    // x86_64::instructions::interrupts::without_interrupts(|| unsafe {
    //     for device in pci::DEVICES.lock().iter() {
    //         device.init(&mut mapper, &mut frame_allocator);
    //     }
    // });

    //exit_qemu(QemuExitCode::Success);
}

/*
 * TESTING CODE
 */
#[cfg(test)]
use bootloader::{bootinfo::BootInfo, entry_point};
use core::{ops::DerefMut, panic::PanicInfo};
// Entry point for `cargo test`
#[cfg(test)]
entry_point!(kernel_test_main);
#[cfg(test)]
fn kernel_test_main(_boot_info: &'static BootInfo) -> ! {
    unsafe {
        init(_boot_info);
    };

    // Function not visible because gets generated by cargo test
    // automatically
    test_main();
    hlt_loop();
}

// panic hanlder called only in cargo test
#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

// Gets array of functions annotated with #[test_case]
pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    //exit_qemu(QemuExitCode::Success);
}

// Prints panic error and quits qemu
#[allow(unreachable_code)]
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    hlt_loop();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

/* Creates the `Testable` trait
 * which helps printing the test function name
 * in the logs when executing cargo test
 */
pub trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

#[test_case]
#[allow(clippy::eq_op)]
fn trivial_assertion() {
    assert_eq!(1, 1);
}
