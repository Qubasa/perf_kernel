use crate::println;
use core::arch::x86_64::{__cpuid, __rdtscp};

enum CpuidIndex {
    TscInvariant = 0x8000_0007,
}

impl CpuidIndex {
    fn as_u32(self) -> u32 {
        self as u32
    }
}

pub struct Bench {
    start: u64,
    end: u64,
}

impl Bench {
    pub fn start() -> Self {
        Bench { start: rdtsc(), end: 0}
    }

    pub fn end(&mut self) {
        self.end = rdtsc();
        let diff = self.end - self.start;

        println!("\nCycles needed: {}", diff);
    }
}


#[inline]
pub fn rdtsc() -> u64 {
    let mut x: u32 = 0;
    unsafe { __rdtscp(&mut x as *mut u32) }
}

pub fn init() {
    let res = unsafe { __cpuid(CpuidIndex::TscInvariant.as_u32()) };

    let tsc_invariant = res.edx & (1 << 8);
    if tsc_invariant == 0 {
        log::warn!("rtdsc does not increment at a fixed rate");
    }
}
