use crate::println;
use core::arch::x86_64::{__cpuid};
use crate::time::{rdtsc, elapsed};

pub enum CpuidIndex {
    TscInvariant = 0x8000_0007,
}

impl CpuidIndex {
    fn as_u32(self) -> u32 {
        self as u32
    }
}

pub struct Bench {
    start: u64,
}

impl Bench {
    pub fn start() -> Self {
        Bench { start: rdtsc()}
    }

    pub fn end(&mut self) {
        let diff = elapsed(self.start);

        println!("\nSeconds needed: {}", diff);
    }
}

pub fn overflow() {
    let a: [u8; 0x1000] = [0; 0x1000];
    let mut x: u64 = 0;
    unsafe {
        asm!("mov {}, rsp", out(reg) x);
    }
    log::info!("Stack ptr: {:#x}", x);
    black_box(a);
    overflow();
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



pub fn check_support() {
    let res = unsafe { __cpuid(CpuidIndex::TscInvariant.as_u32()) };

    let tsc_invariant = res.edx & (1 << 8);
    if tsc_invariant == 0 {
        log::warn!("rtdsc does not increment at a fixed rate");
    }
}

// TODO: When threading is implemented add a counter where execution time is spent most of the time
// TODO: use ibs execution sampling
// Use the core performance counters using rdpmc to measure:
// L2 cache misses
// Make debug information perf compatible!
// https://perf.wiki.kernel.org/index.php/Main_Page
// https://github.com/torvalds/linux/tree/master/tools/perf
