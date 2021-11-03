#[allow(dead_code)]
pub unsafe fn enable_all() {
    use core::arch::x86::{__cpuid, __cpuid_count};
    use x86::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

    // Enable legacy sse instruction set
    {
        // Check the SSE3 instruction set
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 0) == 0 {
            panic!("Processor does not support the sse3 instruction set");
        }

        // Check the floating point instruction set
        let res = __cpuid(0x0000_0001);
        if res.edx & (1 << 0) == 0 {
            panic!("Processor does not support the x87 / floating point instruction set");
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
            flags.remove(Cr0Flags::EMULATE_COPROCESSOR);
            flags.set(Cr0Flags::MONITOR_COPROCESSOR, true);
            flags.remove(Cr0Flags::TASK_SWITCHED);
            Cr0::write(flags);
        }
    }

    // Enable extended SSE instructions
    {
        // Check for xsave instruction
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 26) == 0 {
            log::error!("Processor does not support XSAVE / XSTOR instruction");
            panic!("Processor does not support extended SSE instructions");
        }

        // Check for optional xsaveopt instruction
        let res = __cpuid_count(0x0000_000D, 1);
        if res.eax & (1 << 0) == 0 {
            log::warn!("SSE instruction XSAVEOPT is not supported");
        }

        // Checks for x87, legacy SSE, MMX and extended SSE support
        let res = __cpuid_count(0x0000_000D, 0);
        if res.eax & 0b111 != 0b111 {
            panic!("Processor does not support extended SSE instructions");
        }

        // Enable extended SSE, AVX instruction set
        {
            let mut flags = Cr4::read();
            flags.set(Cr4Flags::OSXSAVE, true);
            Cr4::write(Cr4Flags::OSXSAVE);
        }

        // Check the SSSE3 instruction set
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 9) == 0 {
            log::warn!("Processor does not support the SSSE3 instruction set");
        }

        // Check the SSSE4.1 instruction set
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 19) == 0 {
            log::warn!("Processor does not support the SSE4.1 instruction set");
        }

        // Check the SSE4A instruction set
        let res = __cpuid(0x8000_0001);
        if res.ecx & (1 << 6) == 0 {
            log::warn!("Processor does not support the SSE4A instruction set");
        }

        // Check for AVX support
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 28) == 0 {
            log::warn!("AVX not supported");
        }

        // Check for XOP support
        let res = __cpuid(0x8000_0001);
        if res.ecx & (1 << 11) == 0 {
            log::debug!("AMD XOP not supported");
        }

        // Check for FMA support
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 12) == 0 {
            log::debug!("FMA not supported");
        }

        // Check for FMA4 support
        let res = __cpuid(0x8000_0001);
        if res.ecx & (1 << 16) == 0 {
            log::debug!("FMA4 not supported");
        }

        // Check for BMI1 support
        let res = __cpuid_count(0x0000_0007, 0);
        if res.ebx & (1 << 3) == 0 {
            log::debug!("BMI1 not supported");
        }

        // Check for BMI2 support
        let res = __cpuid_count(0x0000_0007, 0);
        if res.ebx & (1 << 8) == 0 {
            log::debug!("BMI2 not supported");
        }

        // Check for TBM support
        let res = __cpuid(0x8000_0001);
        if res.ecx & (1 << 21) == 0 {
            log::debug!("TBM not supported");
        }

        // Enable instruction extension
        use x86::registers::xcontrol::{XCr0, XCr0Flags};
        let mut flags = XCr0::read();
        flags.set(XCr0Flags::SSE, true);
        flags.set(XCr0Flags::AVX, true);
        XCr0::write(flags);
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
