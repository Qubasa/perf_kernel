use crate::println;
use crate::time::{elapsed, rdtsc};
use core::arch::x86_64::__cpuid;

pub enum CpuidIndex {
    TscInvariant = 0x8000_0007,
    Rdtscp = 0x8000_0001,
    TLBInfo = 0x8000_0005,
    TLBInfo1GbPages = 0x8000_0019,
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
        Bench { start: rdtsc() }
    }

    pub fn end(&mut self) {
        let diff = elapsed(self.start);

        println!("\nSeconds needed: {}", diff);
    }
}

pub fn overflow() {
    let a: [u8; 0x1000] = [0; 0x1000];
    let mut x: u64;
    unsafe {
        asm!("mov {}, rsp", out(reg) x);
    }
    log::info!("Stack ptr: {:#x}", x);
    core::hint::black_box(a);
    overflow();
}

pub fn max_num_4kib_pages() -> u8 {
    let res = unsafe { __cpuid(CpuidIndex::TLBInfo.as_u32()) };
    return ((res.ebx & (0xff << 16)) >> 16) as u8;
}

pub fn max_num_2mib_pages() -> u8 {
    let res = unsafe { __cpuid(CpuidIndex::TLBInfo.as_u32()) };
    return ((res.eax & (0xff << 16)) >> 16) as u8;
}

pub fn max_num_1gib_pages() -> u16 {
    let res = unsafe { __cpuid(CpuidIndex::TLBInfo1GbPages.as_u32()) };
    return ((res.eax & (0xfff << 16)) >> 16) as u16;
}

pub fn check_support() {
    let res = unsafe { __cpuid(CpuidIndex::TscInvariant.as_u32()) };

    let tsc_invariant = res.edx & (1 << 8);
    if tsc_invariant == 0 {
        log::warn!("rtdsc does not increment at a fixed rate");
    }

    let res = unsafe { __cpuid(CpuidIndex::Rdtscp.as_u32()) };
    if res.edx == 0 {
        panic!("Rdtscp instruction is not supported");
    }

    log::info!("max num 1Gib pages: {}", max_num_1gib_pages());
    log::info!("max num 2Mib pages: {}", max_num_2mib_pages());
    log::info!("max num 4Kib pages: {}", max_num_4kib_pages());
}

// TODO: When threading is implemented add a counter where execution time is spent most of the time
// TODO: use ibs execution sampling
// Use the core performance counters using rdpmc to measure:
// L2 cache misses
// Make debug information perf compatible!
// https://perf.wiki.kernel.org/index.php/Main_Page
// https://github.com/torvalds/linux/tree/master/tools/perf
