use core::{
    future::Future,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

use alloc::collections::btree_map::BTreeMap;

use crate::{host, interrupts::IsrRegisterFile, prelude::*};

static PROFILING: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static COUNTS: OnceLock<&'static [AtomicUsize]> = OnceLock::new();
static USER_COUNT: AtomicUsize = AtomicUsize::new(0);

#[core_local]
static MUTE_CORE: AtomicBool = AtomicBool::new(false);

extern "C" {
    static mut _stext: u8;
    static mut _etext: u8;
}

pub(crate) unsafe fn init() {
    COUNTS.get_or_init(|| {
        let size = (&raw const _etext).offset_from_unsigned(&raw const _stext);
        let buffer = Box::new_zeroed_slice(size).assume_init();
        Box::leak(buffer)
    });
}

pub fn begin() {
    PROFILING.store(true, Ordering::SeqCst);
}

pub fn end() {
    PROFILING.store(false, Ordering::SeqCst);
    while ACTIVE.load(Ordering::SeqCst) != 0 {
        core::hint::spin_loop();
    }
}

pub fn muted<T, F: FnOnce() -> T>(f: F) -> T {
    let old = MUTE_CORE.load(Ordering::SeqCst);
    MUTE_CORE.store(true, Ordering::SeqCst);
    let result = f();
    MUTE_CORE.store(old, Ordering::SeqCst);
    result
}

pub async fn muted_async<T, Fut: Future<Output = T>, F: FnOnce() -> Fut>(f: F) -> T {
    let old = MUTE_CORE.load(Ordering::SeqCst);
    MUTE_CORE.store(true, Ordering::SeqCst);
    let result = f().await;
    MUTE_CORE.store(old, Ordering::SeqCst);
    result
}

pub fn mute_core() {
    MUTE_CORE.store(true, Ordering::SeqCst);
}

pub fn unmute_core() {
    MUTE_CORE.store(false, Ordering::SeqCst);
}

pub fn reset() {
    end();
    for count in *COUNTS {
        count.store(0, Ordering::SeqCst);
    }
    USER_COUNT.store(0, Ordering::SeqCst);
}

pub(crate) fn tick(registers: &IsrRegisterFile) {
    ACTIVE.fetch_add(1, Ordering::SeqCst);
    if !PROFILING.load(Ordering::SeqCst) || MUTE_CORE.load(Ordering::SeqCst) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        return;
    }
    if registers.cs & 0b11 == 0b11 {
        // user mode
        USER_COUNT.fetch_add(1, Ordering::SeqCst);
    } else {
        let start = &raw const _stext;
        let end = &raw const _etext;
        let rip = registers.rip as *const u8;
        if rip < start || rip >= end {
            ACTIVE.fetch_sub(1, Ordering::SeqCst);
            log::error!("instruction pointer {rip:p} was not within kernel");
            crate::exit(1);
            // return;
        }

        unsafe {
            let offset = rip.offset_from_unsigned(start);
            COUNTS[offset].fetch_add(1, Ordering::SeqCst);
        }
    }
    ACTIVE.fetch_sub(1, Ordering::SeqCst);
}

pub fn entries() -> BTreeMap<*const (), usize> {
    ACTIVE.fetch_add(1, Ordering::SeqCst);

    let start = &raw const _stext as *const ();

    let mut entries = BTreeMap::new();
    for (i, count) in COUNTS.iter().enumerate() {
        let count = count.load(Ordering::SeqCst);

        if count > 0 {
            unsafe {
                entries.insert(start.byte_add(i), count);
            }
        }
    }
    let user = USER_COUNT.load(Ordering::SeqCst);
    if user > 0 {
        entries.insert(core::ptr::null(), user);
    }
    ACTIVE.fetch_sub(1, Ordering::SeqCst);
    entries
}

pub fn report(entries: &mut [(*const (), usize)]) {
    ACTIVE.fetch_add(1, Ordering::SeqCst);

    let start = &raw const _stext;

    entries.fill((core::ptr::null(), 0));

    for (i, count) in COUNTS.iter().enumerate() {
        let count = count.load(Ordering::SeqCst);

        if count > entries[0].1 {
            unsafe {
                entries[0] = (start.add(i) as *const (), count);
            }
        }

        entries.sort_by_key(|x| x.1);
    }
    entries.reverse();

    ACTIVE.fetch_sub(1, Ordering::SeqCst);
}

pub fn log(count: usize) {
    let mut entries = vec![(core::ptr::null(), count)];
    report(&mut entries);
    for (i, (p, n)) in entries.iter().enumerate() {
        if *n == 0 {
            break;
        }
        let symname = host::symname(*p)
            .map(|(s, i)| alloc::format!("{s}+{i:#x}"))
            .unwrap_or(alloc::format!("{p:p}"));
        log::info!("{i}. {symname} - {n}");
    }
}

#[inline(never)]
pub fn backtrace(f: impl FnMut(*const (), Option<(String, usize)>)) {
    use core::arch::asm;
    let mut rbp: *const usize;
    unsafe {
        asm!("mov {rbp}, rbp", rbp=out(reg)rbp);
        backtrace_from(rbp, f);
    }
}

#[inline(never)]
/// # Safety
///
/// The value of `rbp` must be a valid base/frame pointer from which to backtrace.
pub unsafe fn backtrace_from(
    mut rbp: *const usize,
    mut f: impl FnMut(*const (), Option<(String, usize)>),
) {
    if rbp.is_null() {
        return;
    }
    let mut rip: *const ();
    loop {
        rip = rbp.add(1).read() as *const ();
        rbp = rbp.read() as *const usize;
        if rbp.is_null() {
            break;
        }
        f(rip, crate::host::symname(rip));
    }
}
