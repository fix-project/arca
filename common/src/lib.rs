use std::arch::x86_64::{_mm_lfence, _mm_mfence, _rdtsc};
use std::sync::atomic::{AtomicU64, AtomicU32};

#[repr(C)]
pub struct ShmHeader {
    pub start_index: AtomicU64,
    pub end_index: AtomicU64,
    pub transfer_started: AtomicU32,
}

#[inline]
pub fn read_tsc() -> u64 {
    unsafe {
        _mm_mfence();
        _mm_lfence();
        let tsc = _rdtsc();
        _mm_lfence();
        tsc
    }
}
