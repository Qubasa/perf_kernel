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

/// A function that is opaque to the optimizer, to allow benchmarks to
/// pretend to use outputs to assist in avoiding dead-code
/// elimination.
///
/// This function is a no-op, and does not even read from `dummy`.
pub fn black_box<T>(dummy: T) -> T {
    // we need to "use" the argument in some way LLVM can't
    // introspect.
    unsafe {asm!("/* {0} */" , in(reg) &dummy)}
    dummy
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
