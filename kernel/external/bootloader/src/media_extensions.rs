use raw_cpuid::CpuId;
use x86::registers::xcontrol::{XCr0, XCr0Flags};

#[allow(dead_code)]
pub unsafe fn enable_all() {
    use x86::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};
    let cpuid = CpuId::new();
    let features = cpuid.get_feature_info().unwrap();
    let ext_proc_ids = cpuid
        .get_extended_processor_and_feature_identifiers()
        .unwrap();
    let ext_state = cpuid.get_extended_state_info().unwrap();

    // Enable legacy sse instruction set
    {
        // Check the floating point instruction set
        if !features.has_fpu() {
            log::warn!("Processor does not support the x87 / floating point instruction set");
            return;
        }

        // Check the SSE3 instruction set
        if !features.has_sse() {
            log::warn!("Processor does not support the sse instruction set");
            return;
        }

        // Enable legacy SSE instruction set
        {
            let mut flags = Cr4::read();
            flags.set(Cr4Flags::OSFXSR, true);
            flags.set(Cr4Flags::OSXMMEXCPT_ENABLE, true);
            Cr4::write(flags);
        }

        // Disable exception on execution of x87 and media instructions
        {
            let mut flags = Cr0::read();
            flags.set(Cr0Flags::EMULATE_COPROCESSOR, false);
            flags.set(Cr0Flags::MONITOR_COPROCESSOR, true);
            flags.set(Cr0Flags::TASK_SWITCHED, false);
            Cr0::write(flags);
        }
    }

    // Enable extended SSE instructions
    {
        if !features.has_xsave() {
            log::warn!("Processor does not support XSAVE / XSTOR instruction");
            return;
        }
        if !ext_state.has_xgetbv() {
            log::warn!("Processor does not support XGETBV instruction");
            return;
        }
        if !ext_state.xcr0_supports_legacy_x87() {
            log::warn!("XCR0 does not support x87 fpu");
            return;
        }
        if !ext_state.xcr0_supports_sse_128() {
            log::warn!("SSE not supported");
            return;
        }
        if !ext_state.xcr0_supports_avx_256() || !features.has_avx() {
            log::warn!("AVX not supported");
            return;
        }

        // Enable XSETBV and XGETBV instructions
        {
            let mut flags = Cr4::read();
            flags.set(Cr4Flags::OSXSAVE, true);
            Cr4::write(flags);
        }

        // Enable extended SSE, AVX instruction set in XCR0
        {
            let mut flags = XCr0::read();
            flags.set(XCr0Flags::X87, true);
            flags.set(XCr0Flags::AVX, true);
            flags.set(XCr0Flags::SSE, true);
            XCr0::write(flags);
        }

        if features.has_ssse3() {
            log::debug!("Processor supports SSSE3");
        }
        if features.has_sse41() {
            log::debug!("Processor supports SSE4.1");
        }
        if features.has_sse42() {
            log::debug!("Processor supports SSE4.2");
        }
        if ext_proc_ids.has_sse4a() {
            log::debug!("Processor supports SSE4A");
        }
    }
}

// #[allow(dead_code)]
// pub unsafe fn test_all_extensions() {
//     use core::arch::x86::*;
//     // SSE v1 test
//     let val: __m256 = _mm256_setzero_ps();
//     let hiaddr = &mut [0f32; 4];
//     let loaddr = &mut [0f32; 4];
//     _mm256_storeu2_m128(hiaddr.as_mut_ptr(), loaddr.as_mut_ptr(), val);
//     core::hint::black_box(val);
//     core::hint::black_box(hiaddr);
//     core::hint::black_box(loaddr);

//     // SSE v2 test
//     let val: __m256d = _mm256_setzero_pd();
//     let hiaddr = &mut [0f64; 2];
//     let loaddr = &mut [0f64; 2];
//     _mm256_storeu2_m128d(hiaddr.as_mut_ptr(), loaddr.as_mut_ptr(), val);
//     core::hint::black_box(val);
//     core::hint::black_box(hiaddr);
//     core::hint::black_box(loaddr);

//     // SSE v3 test
//     let val: __m128i = _mm_setzero_si128();
//     let res = _mm_abs_epi8(val);
//     core::hint::black_box(res);

//     // SSE v4.2 test
//     let val1: __m128i = _mm_setzero_si128();
//     let val2: __m128i = _mm_setzero_si128();
//     let res = _mm_cmpgt_epi64(val1, val2);
//     core::hint::black_box(res);

//     // AVX
//     _mm256_zeroall();

//     // AVX 256
//     let val1: __m256i = _mm256_setzero_si256();
//     let val2: __m256i = _mm256_setzero_si256();
//     let res = _mm256_xor_si256(val1, val2);
//     core::hint::black_box(res);
// }
