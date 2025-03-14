use core::{
    arch::{asm, global_asm},
    sync::atomic::AtomicU64,
    time::Duration,
};

#[no_mangle]
static TSC_SYNCHRONIZE: AtomicU64 = AtomicU64::new(0);

static mut TSC_OFFSETS: [u64; 512] = [0; 512];

#[core_local]
static mut TSC_FREQUENCY_MHZ: u32 = 0;

extern "C" {
    fn sync_and_rdtsc(ncores: usize) -> u64;
}

global_asm!(
    "
.extern TSC_SYNCHRONIZE
.globl sync_and_rdtsc
sync_and_rdtsc:
    mfence
    lfence
    # synchronize all cores
    lock inc qword ptr TSC_SYNCHRONIZE[rip]
0:
    cmp TSC_SYNCHRONIZE[rip], rdi
    jne 0b
    # synchronized: read TSC
    mfence
    lfence
    rdtsc
    lfence
    shr rdx, 32
    or rax, rdx
    ret
"
);

pub(crate) unsafe fn init() {
    if crate::coreid() == 0 {
        let x = core::arch::x86_64::__cpuid(0x80000001);
        assert!((x.edx >> 27) & 1 == 1, "RDTSCP is not supported!");
        let x = core::arch::x86_64::__cpuid(0x80000007);
        assert!((x.edx >> 8) & 1 == 1, "Invariant TSC is not supported!");
    }
    let tsc = sync_and_rdtsc(crate::ncores());
    let id = crate::coreid();
    TSC_OFFSETS[id as usize] = tsc;
    // let result = core::arch::x86_64::__cpuid(0x40000010);
    // TODO: this is hardcoded in the runtime right now
    let khz = u32::MAX / 1000;
    *TSC_FREQUENCY_MHZ = khz;
    assert!(*TSC_FREQUENCY_MHZ != 0);
    asm!("wrmsr", in("ecx")0xC0000103u32, in("edx")0, in("eax")id);
}

#[inline]
pub fn read_cycles_raw() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

#[inline]
pub fn read_cycles() -> u64 {
    unsafe {
        let mut core: u32 = 0;
        let mut result = core::arch::x86_64::__rdtscp(&mut core);
        result -= TSC_OFFSETS[core as usize];
        core::arch::x86_64::_mm_lfence();
        result
    }
}

#[inline]
pub fn cycles_to_duration(cycles: u64) -> Duration {
    let ns = cycles as u128 * 1_000_000u128 / unsafe { *TSC_FREQUENCY_MHZ } as u128;
    Duration::from_nanos(ns as u64)
}

#[inline]
pub fn read_raw() -> Duration {
    cycles_to_duration(read_cycles_raw())
}

#[inline]
pub fn read() -> Duration {
    cycles_to_duration(read_cycles())
}

#[inline]
pub fn time(mut f: impl FnMut()) -> Duration {
    let start = read_cycles();
    unsafe {
        core::arch::x86_64::_mm_lfence();
    }
    f();
    unsafe {
        core::arch::x86_64::_mm_mfence();
    }
    let end = read_cycles();
    cycles_to_duration(end - start)
}

#[inline]
pub fn frequency() -> f64 {
    unsafe { *TSC_FREQUENCY_MHZ as f64 * 1e6 }
}
