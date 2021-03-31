pub unsafe fn enable_all() {
    use core::arch::x86::{__cpuid, __cpuid_count};
    use x86::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};

    // Enable legacy sse instruction set
    {
        // Check the sse3 instruction set
        let res = __cpuid(0x0000_0001);
        if res.ecx & (1 << 9) == 0 {
            panic!("Processor does not support the sse3 instruction set");
        }

        // Check the floating point instruction set
        let res = __cpuid(0x0000_0001);
        if res.edx & (1 << 0) == 0 {
            panic!("Processor does not support the x87 / floating point instruction set");
        }

        // Enable legacy sse instruction set
        {
            let mut flags = Cr4::read();
            flags.set(Cr4Flags::OSFXSR, true);
            Cr4::write(flags);
        }

        // Disable exception on x87 and media instructions
        {
            let mut flags = Cr0::read();
            flags.remove(Cr0Flags::EMULATE_COPROCESSOR);
            flags.set(Cr0Flags::MONITOR_COPROCESSOR, true);
            // Disable lazy FPU switching as the kernel will use
            // media extensions
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
        if res.eax & (1<<0) == 0 {
            log::warn!("SSE instruction XSAVEOPT is not supported");
        }

        // Checks for x87, legacy SSE, MMX and extended SSE support
        let res = __cpuid_count(0x0000_000D, 0);
        if res.eax & 0b111 != 0b111 {
            panic!("Processor does not support extended SSE instructions");
        }

        // Enable extended sse instruction set
        {
            let mut flags = Cr4::read();
            flags.set(Cr4Flags::OSXSAVE, true);
            Cr4::write(Cr4Flags::OSXSAVE);
        }

        let res = __cpuid_count(0x0000_000D, 0);
        let XFeatureEnabledSizeMax = res.ebx;
        log::info!("Memory save area size for XSAVE/XRSTOR: {} bytes", XFeatureEnabledSizeMax);

        use x86::registers::xcontrol::{XCr0, XCr0Flags};
        let mut flags = XCr0::read();
        flags.set(XCr0Flags::SSE, true);
        flags.set(XCr0Flags::YMM, true);
        XCr0::write(flags);
        log::info!("XCr0Flags: {:#?}", flags);
        log::info!("XCr0Flags raw: {:#?}", XCr0::read_raw());

        use core::arch::x86::*;
        // FPU test
        let val = 0.5;
        log::info!("FPU 0.5/2 = {}", 0.5/2.0);

        // SSE v1 test
        let val: __m256 = _mm256_setzero_ps();
        let hiaddr = &mut [0f32; 4];
        let loaddr = &mut [0f32; 4];
        _mm256_storeu2_m128(hiaddr.as_mut_ptr(), loaddr.as_mut_ptr(), val);
        core::hint::black_box(val);
        core::hint::black_box(hiaddr);
        core::hint::black_box(loaddr);

        // SSE v2 test
        let val: __m256d = _mm256_setzero_pd();
        let hiaddr = &mut [0f64; 2];
        let loaddr = &mut [0f64; 2];
        _mm256_storeu2_m128d(hiaddr.as_mut_ptr(), loaddr.as_mut_ptr(), val);
        core::hint::black_box(val);
        core::hint::black_box(hiaddr);
        core::hint::black_box(loaddr);

        // SSE v3 test
        let val: __m128i = _mm_setzero_si128();
        let res = _mm_abs_epi8(val);
        core::hint::black_box(res);


        // SSE v4.2 test
        let val1: __m128i = _mm_setzero_si128();
        let val2: __m128i = _mm_setzero_si128();
        let res = _mm_cmpgt_epi64(val1, val2);
        core::hint::black_box(res);
    }
}
