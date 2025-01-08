// https://www.kernel.org/doc/html/v5.9/virt/kvm/msr.html
use core::{
    arch::asm,
    cell::LazyCell,
    ptr::addr_of,
    sync::atomic::{AtomicPtr, Ordering},
    time::Duration,
};

use alloc::boxed::Box;
use time::OffsetDateTime;

use crate::vm;

static BOOT_TIME: AtomicPtr<WallClock> = AtomicPtr::new(core::ptr::null_mut());

#[core_local]
static CPU_TIME_INFO: LazyCell<Box<CpuTimeInfo>> = LazyCell::new(Default::default);

#[repr(C, packed)]
#[derive(Debug, Default, Copy, Clone)]
struct WallClock {
    version: u32,
    sec: u32,
    nsec: u32,
}

#[repr(C, packed)]
#[derive(Debug, Default, Copy, Clone)]
struct CpuTimeInfo {
    version: u32,
    pad0: u32,
    tsc_timestamp: u64,
    system_time: u64,
    tsc_to_system_mul: u32,
    tsc_shift: i8,
    flags: u8,
    pad: [u8; 2],
}
const _: () = assert!(core::mem::size_of::<CpuTimeInfo>() == 32);

pub(crate) unsafe fn init() {
    let x = core::arch::x86_64::__cpuid(0x40000001);
    assert!((x.eax >> 3) & 1 == 1, "KVM Clock is not supported!");

    if BOOT_TIME.load(Ordering::SeqCst).is_null() {
        let boot_time: *mut WallClock = Box::into_raw(Default::default());
        let value = vm::ka2pa(boot_time) as u64;
        asm!("wrmsr", in("edx") value>> 32, in("eax") value, in("ecx") 0x4b564d00);
        if BOOT_TIME
            .compare_exchange(
                core::ptr::null_mut(),
                boot_time,
                Ordering::SeqCst,
                Ordering::SeqCst,
            )
            .is_err()
        {
            let _ = Box::from_raw(boot_time);
        }
    }

    let value = vm::ka2pa(CPU_TIME_INFO.as_ref()) as u64 | 1;
    asm!("wrmsr", in("edx") value >> 32, in("eax") value, in("ecx") 0x4b564d01);
}

#[inline]
fn read_boot_time() -> WallClock {
    unsafe {
        let boot_time: *mut WallClock = BOOT_TIME.load(Ordering::SeqCst);
        loop {
            let v0 = addr_of!((*boot_time).version).read_volatile();
            let data = *boot_time;
            let v1 = addr_of!((*boot_time).version).read_volatile();
            if v0 != v1 || (v0 % 2) != 0 || v0 == 0 {
                core::arch::x86_64::_mm_pause();
                continue;
            }
            return data;
        }
    }
}

#[inline]
fn read_current() -> (CpuTimeInfo, u64) {
    let cpu_time = (**CPU_TIME_INFO).as_ref();
    unsafe {
        loop {
            let v0 = addr_of!(cpu_time.version).read_volatile();
            let data = *cpu_time;
            let tsc = core::arch::x86_64::_rdtsc();
            let v1 = addr_of!(cpu_time.version).read_volatile();
            if v0 != v1 || (v0 % 2) != 0 || v0 == 0 {
                core::arch::x86_64::_mm_pause();
                continue;
            }
            return (data, tsc);
        }
    }
}

#[inline]
fn info_and_tsc_to_duration(info: CpuTimeInfo, tsc: u64) -> Duration {
    let time = tsc.wrapping_sub(info.tsc_timestamp);
    let time = if info.tsc_shift >= 0 {
        time << info.tsc_shift
    } else {
        time >> -info.tsc_shift
    };
    let time = (time.wrapping_mul(info.tsc_to_system_mul as u64)) >> 32;
    let time = time.wrapping_add(info.system_time);
    Duration::from_nanos(time)
}

#[inline]
pub fn time_since_boot() -> Duration {
    let (info, tsc) = read_current();
    info_and_tsc_to_duration(info, tsc)
}

#[inline]
pub fn wall_clock_time() -> OffsetDateTime {
    let boot_time = read_boot_time();
    let boot_time =
        Duration::from_secs(boot_time.sec as u64) + Duration::from_nanos(boot_time.nsec as u64);
    let current_time = time_since_boot();
    let now = boot_time + current_time;
    let sec = now.as_secs();
    let nsec = now.subsec_nanos();
    let timestamp = (sec as i128) * 1_000_000_000 + nsec as i128;
    OffsetDateTime::from_unix_timestamp_nanos(timestamp).unwrap()
}

#[inline]
pub fn time(f: impl Fn()) -> Duration {
    unsafe {
        core::arch::x86_64::_mm_lfence();
    };
    let (start_info, start_tsc) = read_current();
    f();
    unsafe {
        core::arch::x86_64::_mm_mfence();
        core::arch::x86_64::_mm_lfence();
    };
    let (end_info, end_tsc) = read_current();
    info_and_tsc_to_duration(end_info, end_tsc) - info_and_tsc_to_duration(start_info, start_tsc)
}
