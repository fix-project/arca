/// 16 YMM registers × 32 bytes = 512 bytes
const YMM_ALL_BYTES: usize = 16 * 32;

#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
unsafe fn ymm_all_store(out: &mut [u8; YMM_ALL_BYTES]) {
    let base = out.as_mut_ptr() as usize;
    core::arch::asm!(
        "vmovdqu [{base}], ymm0",
        "vmovdqu [{base} + 32], ymm1",
        "vmovdqu [{base} + 64], ymm2",
        "vmovdqu [{base} + 96], ymm3",
        "vmovdqu [{base} + 128], ymm4",
        "vmovdqu [{base} + 160], ymm5",
        "vmovdqu [{base} + 192], ymm6",
        "vmovdqu [{base} + 224], ymm7",
        "vmovdqu [{base} + 256], ymm8",
        "vmovdqu [{base} + 288], ymm9",
        "vmovdqu [{base} + 320], ymm10",
        "vmovdqu [{base} + 352], ymm11",
        "vmovdqu [{base} + 384], ymm12",
        "vmovdqu [{base} + 416], ymm13",
        "vmovdqu [{base} + 448], ymm14",
        "vmovdqu [{base} + 480], ymm15",
        base = in(reg) base,
        options(nostack, preserves_flags)
    );
}

#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
unsafe fn ymm_all_load(inp: &[u8; YMM_ALL_BYTES]) {
    let base = inp.as_ptr() as usize;
    core::arch::asm!(
        "vmovdqu ymm0, [{base}]",
        "vmovdqu ymm1, [{base} + 32]",
        "vmovdqu ymm2, [{base} + 64]",
        "vmovdqu ymm3, [{base} + 96]",
        "vmovdqu ymm4, [{base} + 128]",
        "vmovdqu ymm5, [{base} + 160]",
        "vmovdqu ymm6, [{base} + 192]",
        "vmovdqu ymm7, [{base} + 224]",
        "vmovdqu ymm8, [{base} + 256]",
        "vmovdqu ymm9, [{base} + 288]",
        "vmovdqu ymm10, [{base} + 320]",
        "vmovdqu ymm11, [{base} + 352]",
        "vmovdqu ymm12, [{base} + 384]",
        "vmovdqu ymm13, [{base} + 416]",
        "vmovdqu ymm14, [{base} + 448]",
        "vmovdqu ymm15, [{base} + 480]",
        base = in(reg) base,
        options(nostack, preserves_flags)
    );
}

/// Build a pattern where YMMi has byte value (base + i) throughout.
fn ymm_pattern(base: u8) -> [u8; YMM_ALL_BYTES] {
    let mut buf = [0u8; YMM_ALL_BYTES];
    for i in 0..16usize {
        let fill = base.wrapping_add(i as u8);
        buf[i * 32..(i + 1) * 32].fill(fill);
    }
    buf
}

/// Build a pattern where each byte is unique and sequential.
fn ymm_pattern_sequential(start: u8) -> [u8; YMM_ALL_BYTES] {
    let mut buf = [0u8; YMM_ALL_BYTES];
    for i in 0..YMM_ALL_BYTES {
        buf[i] = start.wrapping_add(i as u8);
    }
    buf
}

#[test]
fn test_cpuid_xsave_avx_exposed() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::{__cpuid, _xgetbv};

        let leaf1 = __cpuid(1);
        let ecx = leaf1.ecx;

        let xsave   = (ecx >> 26) & 1 == 1;
        let osxsave = (ecx >> 27) & 1 == 1;
        let avx     = (ecx >> 28) & 1 == 1;

        log::info!("CPUID.1:ECX = {:#010x}", ecx);
        log::info!("  XSAVE   (ECX[26]) = {}", xsave);
        log::info!("  OSXSAVE (ECX[27]) = {}", osxsave);
        log::info!("  AVX     (ECX[28]) = {}", avx);

        if osxsave {
            // If CR4.OSXSAVE isn't set, xgetbv would #UD.
            // If your kernel already uses AVX successfully, it's likely set,
            // but logging this helps catch mismatches.
            let xcr0 = _xgetbv(0);
            log::info!("XCR0 = {:#x} (x87={}, sse={}, avx={})",
                xcr0,
                (xcr0 & 0x1) != 0,
                (xcr0 & 0x2) != 0,
                (xcr0 & 0x4) != 0
            );
        } else {
            log::info!("OSXSAVE not exposed by CPUID; guest cannot legally use xgetbv/xsetbv.");
        }

        // Optional: fail fast if the VMM doesn't expose what you need.
        // Comment these out if you just want diagnostics.
        assert!(xsave, "VMM/guest CPUID does not expose XSAVE");
        assert!(osxsave, "VMM/guest CPUID does not expose OSXSAVE");
        assert!(avx, "VMM/guest CPUID does not expose AVX");
    }
}

#[test]
fn test_avx_work_on_host() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;

        let a = _mm256_set_ps(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0);
        let b = _mm256_set_ps(8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0);
        let result = _mm256_add_ps(a, b);

        let mut output: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(output.as_mut_ptr(), result);

        for &val in &output {
            assert_eq!(val, 9.0, "AVX addition failed");
        }

        log::info!("AVX 256-bit addition passed");

    }
    
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;

        let a = _mm256_set_ps(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0);
        let b = _mm256_set_ps(2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0);
        let mul = _mm256_mul_ps(a, b);
        let div = _mm256_div_ps(mul, b);

        let mut output: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(output.as_mut_ptr(), div);

        let expected = [8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0];
        for (i, (&got, &exp)) in output.iter().zip(expected.iter()).enumerate() {
            assert_eq!(got, exp, "AVX mul/div failed at index {}", i);
        }

        log::info!("AVX multiply/divide passed");
    }

    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;

        let zero = _mm256_setzero_ps();
        let mut output: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(output.as_mut_ptr(), zero);

        for &val in &output {
            assert_eq!(val, 0.0, "AVX zero failed");
        }

        let all_fives = _mm256_set1_ps(5.0);
        _mm256_storeu_ps(output.as_mut_ptr(), all_fives);

        for &val in &output {
            assert_eq!(val, 5.0, "AVX set1 failed");
        }

        log::info!("AVX zero and set1 passed");
    }
}

#[test]
fn test_avx_state_preservation_across_unload() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use crate::cpu::CPU;
        use crate::types::arca::Arca;

        let mut cpu = CPU.borrow_mut();

        // Two Arcas with distinct YMM patterns (all 16 YMMs)
        let pat_a = ymm_pattern_sequential(0);
        let pat_b = ymm_pattern_sequential(128);

        let mut arca_a = Arca::new();
        let mut arca_b = Arca::new();

        // Load A, set all YMM to pat_a, unload
        let mut loaded = arca_a.load(&mut cpu);
        ymm_all_load(&pat_a);
        arca_a = loaded.unload();

        // Load B, set all YMM to pat_b, unload
        loaded = arca_b.load(&mut cpu);
        ymm_all_load(&pat_b);
        arca_b = loaded.unload();

        // Switch back and forth 3 times, verify both preserve all YMMs
        for i in 0..3 {
            // Load A → verify all YMMs match pat_a
            loaded = arca_a.load(&mut cpu);
            let mut got = [0u8; YMM_ALL_BYTES];
            ymm_all_store(&mut got);
            assert_eq!(got, pat_a, "Arca A lost YMM state on cycle {}", i);
            arca_a = loaded.unload();

            // Load B → verify all YMMs match pat_b
            loaded = arca_b.load(&mut cpu);
            ymm_all_store(&mut got);
            assert_eq!(got, pat_b, "Arca B lost YMM state on cycle {}", i);
            arca_b = loaded.unload();
        }

        log::info!("All 16 YMMs preserved for both Arcas across 3 load/unload cycles");
    }
}

#[test]
fn test_avx_state_swap_between_arcas() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use crate::cpu::CPU;
        use crate::types::arca::Arca;

        let mut cpu = CPU.borrow_mut();

        let pat_a = ymm_pattern(0x11);
        let pat_b = ymm_pattern(0x22);

        let arca_a = Arca::new();
        let mut arca_b = Arca::new();

        let mut loaded = arca_a.load(&mut cpu);
        ymm_all_load(&pat_a);

        loaded.swap(&mut arca_b);
        ymm_all_load(&pat_b);

        // Switch back and forth 3 times, verify both preserve all YMMs
        for i in 0..3 {
            loaded.swap(&mut arca_b);
            let mut got = [0u8; YMM_ALL_BYTES];
            ymm_all_store(&mut got);
            assert_eq!(got, pat_a, "Arca A lost YMM state on swap cycle {}", i);

            loaded.swap(&mut arca_b);
            ymm_all_store(&mut got);
            assert_eq!(got, pat_b, "Arca B lost YMM state on swap cycle {}", i);
        }

        let _ = loaded.unload();
        log::info!("All 16 YMMs swap verified for both Arcas across 3 cycles");
    }
}

#[test]
fn test_fsbase_preservation() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use crate::cpu::CPU;
        use crate::types::arca::Arca;

        let mut cpu = CPU.borrow_mut();

        let arca = Arca::new();
        let loaded = arca.load(&mut cpu);

        // Set a known fsbase; unload captures it via rdfsbase.
        // Must be a canonical x86-64 address to avoid #GP.
        const EXPECTED_FSBASE: u64 = 0xFFFF_8000_0000_1000;
        core::arch::asm!(
            "wrfsbase {base}",
            base = in(reg) EXPECTED_FSBASE,
            options(nostack, preserves_flags)
        );

        let arca = loaded.unload();

        // Reload and verify fsbase was preserved
        let loaded = arca.load(&mut cpu);
        let mut actual: u64 = 0;
        core::arch::asm!(
            "rdfsbase {base}",
            base = out(reg) actual,
            options(nostack, preserves_flags)
        );

        assert_eq!(actual, EXPECTED_FSBASE, "fsbase not preserved across load/unload");

        let _ = loaded.unload();
        log::info!("fsbase preservation verified");
    }
}

#[test]
fn test_xstate_stable_across_multiple_cycles() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use crate::cpu::CPU;
        use crate::types::arca::Arca;

        let mut cpu = CPU.borrow_mut();

        let arca = Arca::new();
        let mut loaded = arca.load(&mut cpu);

        let pattern = ymm_pattern_sequential(1);
        ymm_all_load(&pattern);

        let mut arca = loaded.unload();

        // Unload/reload cycle × 5 (arca stays on the stack, we just load/unload repeatedly)
        for _ in 0..5 {
            loaded = arca.load(&mut cpu);
            arca = loaded.unload();
        }

        // Final reload and verify all YMMs unchanged
        loaded = arca.load(&mut cpu);
        let mut got = [0u8; YMM_ALL_BYTES];
        ymm_all_store(&mut got);

        assert_eq!(got, pattern, "YMM state corrupted across multiple load/unload cycles");

        let _ = loaded.unload();
        log::info!("All 16 YMMs stable across 5 load/unload cycles");
    }
}

#[test]
fn test_required_bytes_for_avx_mask() {
    #[cfg(target_arch = "x86_64")]
    {
        use crate::types::arca;

        let needed = arca::required_bytes_for_avx_mask();
        // x87+SSE = 512B legacy + 64B header = 576B; AVX adds 256B (16×YMM_Hi128) at offset 576.
        assert!(
            needed >= 576 + 256,
            "mask 0x7 should need at least 832 bytes for x87+SSE+AVX, got {}",
            needed
        );
        assert!(
            needed <= 1024,
            "expected compact x87+SSE+AVX to fit in 1024 bytes, got {}",
            needed
        );
        log::info!("required_bytes_for_avx_mask = {}", needed);
    }
}

#[test]
fn test_xsave_area_alignment() {
    #[cfg(target_arch = "x86_64")]
    {
        use crate::types::arca;

        let align = arca::xsave_area_alignment();
        assert_eq!(align, 64, "XsaveArea must be 64-byte aligned for XSAVE/XRSTOR");
        log::info!("XsaveArea alignment = {} bytes", align);
    }
}

/// Perform VPADDB ymm0, ymm0, ymm0 (byte-wise add: each byte doubled) and leave result in ymm0.
/// This is a real SIMD operation; the modified data stays in the register.
#[cfg(target_arch = "x86_64")]
#[allow(dead_code)]
unsafe fn ymm0_simd_double_bytes() {
    core::arch::asm!(
        "vpaddb ymm0, ymm0, ymm0",
        options(nostack, preserves_flags)
    );
}

#[test]
fn test_simd_result_preserved_across_unload() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use crate::cpu::CPU;
        use crate::types::arca::Arca;

        let mut cpu = CPU.borrow_mut();

        // Start with a pattern: ymm0 gets bytes 0..31, ymm1 gets 32..63, etc.
        let pattern = ymm_pattern_sequential(0);
        // After VPADDB ymm0, ymm0, ymm0: each byte in ymm0 is doubled (mod 256).
        // So bytes 0..31 become 0,2,4,6,...,60,62.
        let mut expected_ymm0 = [0u8; 32];
        for i in 0..32 {
            expected_ymm0[i] = (i as u8).wrapping_mul(2);
        }

        let mut arca = Arca::new();
        let mut loaded = arca.load(&mut cpu);

        ymm_all_load(&pattern);
        ymm0_simd_double_bytes(); // SIMD op: ymm0 = ymm0 + ymm0 (in-place)

        arca = loaded.unload();

        // Reload and verify the SIMD-computed result is still in ymm0
        loaded = arca.load(&mut cpu);
        let mut got = [0u8; YMM_ALL_BYTES];
        ymm_all_store(&mut got);

        assert_eq!(
            &got[0..32],
            &expected_ymm0[..],
            "SIMD result in ymm0 not preserved across unload/reload"
        );
        // ymm1..ymm15 should be unchanged
        assert_eq!(
            &got[32..64],
            &pattern[32..64],
            "ymm1 corrupted when only ymm0 was modified by SIMD"
        );

        let _ = loaded.unload();
        log::info!("SIMD (VPADDB) result preserved in ymm0 across load/unload");
    }
}

#[test]
fn test_avx_state_isolation_between_arcas() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use crate::cpu::CPU;
        use crate::types::arca::Arca;

        let mut cpu = CPU.borrow_mut();

        let arca_a = Arca::new();
        let loaded_a = arca_a.load(&mut cpu);

        let secret = ymm_pattern_sequential(0x5C);
        ymm_all_load(&secret);

        let arca_a = loaded_a.unload();

        let arca_b = Arca::new();
        let loaded_b = arca_b.load(&mut cpu);

        let mut b_seen = [0u8; YMM_ALL_BYTES];
        ymm_all_store(&mut b_seen);

        assert_ne!(b_seen, secret, "Arca B observed Arca A's YMM state (leak)");

        let arca_b = loaded_b.unload();

        let loaded_a = arca_a.load(&mut cpu);
        let mut a_seen = [0u8; YMM_ALL_BYTES];
        ymm_all_store(&mut a_seen);

        assert_eq!(a_seen, secret, "Arca A did not retain its YMM state after reload");

        let _ = loaded_a.unload();
        let _ = arca_b;
        log::info!("All 16 YMMs isolation verified between arcas");
    }
}
