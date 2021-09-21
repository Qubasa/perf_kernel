use core::arch::x86::__cpuid;
use x86::addr::PhysAddr;

use crate::bootinfo;

pub static mut BOOT_INFO: bootinfo::BootInfo = bootinfo::BootInfo::new();

extern "C" {
    pub fn undefined_instruction();
    fn switch_to_long_mode(
        boot_info: &'static bootinfo::BootInfo,
        entry_point: u32,
        stack_addr: u32,
    ) -> !;
}

pub fn num_cores() -> u32 {
    unsafe {
        let res = __cpuid(0x8000_0008);
        return (res.ecx & 0xFF) + 1;
    };
}

pub fn apic_id() -> u8 {
    unsafe {
        let res = __cpuid(0x0000_0001);
        let core_id = (res.ebx >> 24) as u8;
        return core_id;
    };
}

#[no_mangle]
unsafe extern "C" fn smp_main() {
    // Load interrupt handlers for x86 mode
    log::info!("Core {} says hello", apic_id());

    // Load exception handler in case of an error
    crate::interrupts::load_idt();

    // Enable all media extensions
    crate::media_extensions::enable_all();

    // Get stack address for this core
    let stack_addr = BOOT_INFO.cores[apic_id() as usize].stack_start_addr as u32;
    log::info!("Stack addr: {:#x}", stack_addr);

    // Enable mmu features
    // and set cr3 register with memory map
    crate::mmu::setup_mmu(PhysAddr::new(BOOT_INFO.page_table_addr));

    let entry_addr = BOOT_INFO.kernel_entry_addr;

    log::info!("Switching to long mode...");

    switch_to_long_mode(&BOOT_INFO, entry_addr, stack_addr);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }

    panic!("Failed to exit Qemu");
}
