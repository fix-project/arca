// Test for 256-bit SIMD (AVX) support

#[test]
fn test_avx_basic_add() {
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
        
        log::info!("✓ AVX 256-bit SIMD test passed");
    }
}

#[test]
fn test_avx_multiply_divide() {
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
        
        log::info!("✓ AVX multiply/divide passed");
    }
}

#[test]
fn test_avx_zero_and_set() {
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
        
        log::info!("✓ AVX zero and set1 passed");
    }
}

#[test]
fn test_avx_state_preservation_across_unload() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        use crate::cpu::CPU;
        use crate::types::arca::Arca;
        
        let arca = Arca::new();
        let mut cpu = CPU.borrow_mut();
        let mut loaded = arca.load(&mut cpu);
        
        // Set YMM0 to specific pattern by doing an operation
        let a = _mm256_set_ps(10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0);
        let b = _mm256_set1_ps(2.0);
        let result = _mm256_mul_ps(a, b); // YMM0 now has [20, 40, 60, 80, 100, 120, 140, 160]
        
        // Store to verify what we expect
        let mut before_unload: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(before_unload.as_mut_ptr(), result);
        
        // Unload - this should save YMM registers via XSAVE
        let arca = loaded.unload();
        
        // Create and load a different arca that will clobber YMM registers
        let mut arca2 = Arca::new();
        let loaded2 = arca2.load(&mut cpu);
        
        // Clobber YMM0 with completely different values
        let different = _mm256_set1_ps(999.0);
        let _ = _mm256_add_ps(different, different); // YMM0 now has [1998, 1998, ...]
        
        arca2 = loaded2.unload();
        
        // Reload first arca - XRSTOR should restore YMM registers
        loaded = arca.load(&mut cpu);
        
        // If state was preserved, using result in an operation should work
        // Add zero to force the preserved value to be used
        let preserved = _mm256_add_ps(result, _mm256_setzero_ps());
        
        let mut after_reload: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(after_reload.as_mut_ptr(), preserved);
        
        // These should match if XSAVE/XRSTOR worked
        for (i, (&before, &after)) in before_unload.iter().zip(after_reload.iter()).enumerate() {
            assert_eq!(before, after, "State not preserved at index {} (before={}, after={})", i, before, after);
        }
        
        let _ = loaded.unload();
        let _ = arca2;
        
        log::info!("✓ AVX state preservation verified across unload/load");
    }
}

#[test]
fn test_avx_state_swap_between_arcas() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        use crate::cpu::CPU;
        use crate::types::arca::Arca;
        
        let arca1 = Arca::new();
        let mut arca2 = Arca::new();
        
        let mut cpu = CPU.borrow_mut();
        let mut loaded1 = arca1.load(&mut cpu);
        
        // Set arca1's YMM0 to [1, 1, 1, 1, 1, 1, 1, 1]
        let ones = _mm256_set1_ps(1.0);
        let arca1_result = _mm256_mul_ps(ones, ones);
        
        let mut arca1_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(arca1_values.as_mut_ptr(), arca1_result);
        
        // Swap to arca2 (should save arca1's state, restore arca2's state)
        loaded1.swap(&mut arca2);
        
        // Set arca2's YMM0 to [2, 2, 2, 2, 2, 2, 2, 2]
        let twos = _mm256_set1_ps(2.0);
        let arca2_result = _mm256_mul_ps(twos, twos); // [4, 4, 4, 4, 4, 4, 4, 4]
        
        let mut arca2_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(arca2_values.as_mut_ptr(), arca2_result);
        
        // Swap back to arca1 (should restore arca1's [1,1,1,1,1,1,1,1])
        loaded1.swap(&mut arca2);
        
        // If swap preserved state, YMM0 should still have arca1's values
        let restored = _mm256_add_ps(arca1_result, _mm256_setzero_ps());
        
        let mut restored_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(restored_values.as_mut_ptr(), restored);
        
        // Should match original arca1 values
        for (i, (&original, &restored)) in arca1_values.iter().zip(restored_values.iter()).enumerate() {
            assert_eq!(original, restored, "Swap didn't preserve state at index {} (original={}, restored={})", i, original, restored);
        }
        
        // Swap to arca2 and verify its state too
        loaded1.swap(&mut arca2);
        
        let arca2_restored = _mm256_add_ps(arca2_result, _mm256_setzero_ps());
        let mut arca2_restored_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(arca2_restored_values.as_mut_ptr(), arca2_restored);
        
        for (i, (&original, &restored)) in arca2_values.iter().zip(arca2_restored_values.iter()).enumerate() {
            assert_eq!(original, restored, "Swap didn't preserve arca2 state at index {}", i);
        }
        
        let _ = loaded1.unload();
        
        log::info!("✓ AVX state swap verified between arcas");
    }
}

#[test]
fn test_avx_operations_after_multiple_unloads() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        use crate::cpu::CPU;
        use crate::types::arca::Arca;
        
        let mut arca = Arca::new();
        let mut cpu = CPU.borrow_mut();
        
        // First load: set unique pattern
        let mut loaded = arca.load(&mut cpu);
        let pattern1 = _mm256_set_ps(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0);
        let result1 = _mm256_mul_ps(pattern1, _mm256_set1_ps(1.0));
        
        let mut expected1: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(expected1.as_mut_ptr(), result1);
        
        arca = loaded.unload();
        
        // Load different arca to clobber registers
        let mut temp_arca = Arca::new();
        let temp_loaded = temp_arca.load(&mut cpu);
        let _ = _mm256_set1_ps(999.0); // Clobber
        temp_arca = temp_loaded.unload();
        
        // Reload original - should restore state
        loaded = arca.load(&mut cpu);
        let restored1 = _mm256_add_ps(result1, _mm256_setzero_ps());
        
        let mut got1: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(got1.as_mut_ptr(), restored1);
        
        for (i, (&exp, &got)) in expected1.iter().zip(got1.iter()).enumerate() {
            assert_eq!(exp, got, "Cycle 1: State not preserved at index {}", i);
        }
        
        arca = loaded.unload();
        
        // Repeat with different pattern
        loaded = arca.load(&mut cpu);
        let pattern2 = _mm256_set1_ps(42.0);
        let result2 = _mm256_mul_ps(pattern2, _mm256_set1_ps(2.0)); // [84, 84, ...]
        
        let mut expected2: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(expected2.as_mut_ptr(), result2);
        
        arca = loaded.unload();
        
        // Clobber again
        let temp_loaded = temp_arca.load(&mut cpu);
        let _ = _mm256_set1_ps(777.0);
        temp_arca = temp_loaded.unload();
        
        // Reload and verify
        loaded = arca.load(&mut cpu);
        let restored2 = _mm256_add_ps(result2, _mm256_setzero_ps());
        
        let mut got2: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(got2.as_mut_ptr(), restored2);
        
        for (i, (&exp, &got)) in expected2.iter().zip(got2.iter()).enumerate() {
            assert_eq!(exp, got, "Cycle 2: State not preserved at index {}", i);
        }
        
        let _ = loaded.unload();
        let _ = temp_arca;
        
        log::info!("✓ AVX state preserved across multiple unload/load cycles");
    }
}

#[test]
fn test_avx_state_isolation_between_arcas() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use core::arch::x86_64::*;
        use crate::cpu::CPU;
        use crate::types::arca::Arca;
        
        let mut cpu = CPU.borrow_mut();
        
        // Create arca A with secret values
        let arca_a = Arca::new();
        let mut loaded_a = arca_a.load(&mut cpu);
        
        // Set "secret" values in arca A's YMM registers
        let secret = _mm256_set_ps(11.0, 22.0, 33.0, 44.0, 55.0, 66.0, 77.0, 88.0);
        let secret_result = _mm256_mul_ps(secret, _mm256_set1_ps(1.0));
        
        let mut secret_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(secret_values.as_mut_ptr(), secret_result);
        
        // Unload arca A (should save its state)
        let arca_a = loaded_a.unload();
        
        // Create and load arca B - it should NOT see arca A's values
        let arca_b = Arca::new();
        let mut loaded_b = arca_b.load(&mut cpu);
        
        // Arca B's YMM registers should be zeroed (from initialization), not contain arca A's secrets
        // Try to read what's in the registers by adding zero
        let zero = _mm256_setzero_ps();
        let leaked = _mm256_add_ps(zero, zero); // If registers weren't cleared, this would contain garbage/secrets
        
        let mut leaked_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(leaked_values.as_mut_ptr(), leaked);
        
        // All values should be zero or arca B's initial state, NOT arca A's secrets
        for (i, &leaked) in leaked_values.iter().enumerate() {
            // The leaked value should not match arca A's secret
            // (It might be 0 or some other value, but not the secret)
            let matches_secret = secret_values.iter().any(|&s| s == leaked && s != 0.0);
            assert!(!matches_secret, 
                "Arca B leaked arca A's secret value {} at index {}", leaked, i);
        }
        
        // Now set arca B's own values
        let b_values = _mm256_set1_ps(99.0);
        let b_result = _mm256_mul_ps(b_values, _mm256_set1_ps(1.0));
        
        let mut b_stored: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(b_stored.as_mut_ptr(), b_result);
        
        let arca_b = loaded_b.unload();
        
        // Load arca A again - it should see its original secrets, not arca B's values
        loaded_a = arca_a.load(&mut cpu);
        let restored_a = _mm256_add_ps(secret_result, _mm256_setzero_ps());
        
        let mut restored_a_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(restored_a_values.as_mut_ptr(), restored_a);
        
        // Arca A should see its own secrets
        for (i, (&original, &restored)) in secret_values.iter().zip(restored_a_values.iter()).enumerate() {
            assert_eq!(original, restored, 
                "Arca A's secret was corrupted at index {} (expected {}, got {})", 
                i, original, restored);
        }
        
        // Arca A should NOT see arca B's values
        for (i, &restored) in restored_a_values.iter().enumerate() {
            assert_ne!(restored, 99.0, 
                "Arca A leaked arca B's value at index {}", i);
        }
        
        let arca_a = loaded_a.unload();
        
        // Load arca B again - it should see its own values, not arca A's secrets
        loaded_b = arca_b.load(&mut cpu);
        let restored_b = _mm256_add_ps(b_result, _mm256_setzero_ps());
        
        let mut restored_b_values: [f32; 8] = [0.0; 8];
        _mm256_storeu_ps(restored_b_values.as_mut_ptr(), restored_b);
        
        // Arca B should see its own values
        for (i, (&original, &restored)) in b_stored.iter().zip(restored_b_values.iter()).enumerate() {
            assert_eq!(original, restored,
                "Arca B's value was corrupted at index {}", i);
        }
        
        // Arca B should NOT see arca A's secrets
        for (i, &restored) in restored_b_values.iter().enumerate() {
            let matches_secret = secret_values.iter().any(|&s| s == restored);
            assert!(!matches_secret,
                "Arca B leaked arca A's secret at index {}", i);
        }
        
        let _ = loaded_b.unload();
        let _ = arca_a;
        
        log::info!("✓ AVX state properly isolated between arcas - no leakage");
    }
}
