#![optimize(speed)]
use core::{
    arch::{asm, global_asm},
    sync::atomic::AtomicU64,
    time::Duration,
};

#[thread_local]
static mut TSC_FREQUENCY_KHZ: u32 = 0;

#[no_mangle]
static TSC_SYNCHRONIZE: AtomicU64 = AtomicU64::new(0);

static mut TSC_OFFSETS: [u64; 512] = [0; 512];

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
    if crate::cpu_is_bootstrap() {
        let x = core::arch::x86_64::__cpuid(0x80000001);
        assert!((x.edx >> 27) & 1 == 1, "RDTSCP is not supported!");
        let x = core::arch::x86_64::__cpuid(0x80000007);
        assert!((x.edx >> 8) & 1 == 1, "Invariant TSC is not supported!");
    }
    let tsc = sync_and_rdtsc(crate::cpu_ncores());
    TSC_OFFSETS[crate::CPU_ACPI_ID] = tsc;
    let result = core::arch::x86_64::__cpuid(0x40000010);
    TSC_FREQUENCY_KHZ = result.eax;
    assert!(TSC_FREQUENCY_KHZ != 0);
    let id = crate::cpu_acpi_id();
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
        let result = core::arch::x86_64::__rdtscp(&mut core);
        result - TSC_OFFSETS[core as usize]
    }
}

#[inline]
pub fn cycles_to_duration(cycles: u64) -> Duration {
    let ns = unsafe { cycles * 1_000_000 / TSC_FREQUENCY_KHZ as u64 };
    Duration::from_nanos(ns)
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
pub fn time(f: impl Fn()) -> Duration {
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