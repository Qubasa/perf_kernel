use core::arch::x86::{__cpuid};

pub fn num_cores() -> u32 {
    unsafe {
        let res = __cpuid(0x8000_0008);
        return (res.ecx & 0xFF) + 1;
    };
}

pub fn apic_id() -> u8 {
    unsafe {
        let res = __cpuid(0x8000_0008);
        return (res.ebx & (0xFF << 24)) as u8;
    };
}
