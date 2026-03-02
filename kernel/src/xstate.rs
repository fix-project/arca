#[cfg(target_arch = "x86_64")]
pub fn init() {
    unsafe {
        use core::arch::x86_64::{__cpuid, _xgetbv, _xsetbv};

        let ecx = __cpuid(1).ecx;
        let xsave   = ((ecx >> 26) & 1) == 1;
        let osxsave = ((ecx >> 27) & 1) == 1;
        let avx     = ((ecx >> 28) & 1) == 1;

        if !(xsave && osxsave && avx) {
            log::info!("xstate::init: skipping AVX enable (cpuid says xsave={}, osxsave={}, avx={})", xsave, osxsave, avx);
            return;
        }

        _xsetbv(0, 0x7);
        let xcr0 = _xgetbv(0);
        debug_assert!((xcr0 & 0x7) == 0x7);
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn init() {}