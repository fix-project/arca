use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use alloc::collections::btree_map::BTreeMap;

use crate::{interrupts::IsrRegisterFile, prelude::*};

static PROFILING: AtomicBool = AtomicBool::new(false);
static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static COUNTS: OnceLock<&'static [AtomicUsize]> = OnceLock::new();

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

pub fn reset() {
    end();
    for count in *COUNTS {
        count.store(0, Ordering::SeqCst);
    }
}

pub(crate) fn tick(registers: &IsrRegisterFile) {
    ACTIVE.fetch_add(1, Ordering::SeqCst);
    if !PROFILING.load(Ordering::SeqCst) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        return;
    }
    let start = &raw const _stext;
    let end = &raw const _etext;
    let rip = registers.rip as *const u8;
    if rip < start || rip >= end {
        log::warn!("instruction pointer {rip:p} was not within kernel");
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
        return;
    }

    unsafe {
        let offset = rip.offset_from_unsigned(start);
        COUNTS[offset].fetch_add(1, Ordering::SeqCst);
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

#[inline(never)]
pub fn backtrace() {
    use core::arch::asm;
    unsafe {
        let mut rbp: *const usize;
        let mut rip: *const ();
        asm!("mov {rbp}, rbp", rbp=out(reg)rbp);
        log::warn!("rbp: {rbp:p}");
        loop {
            rip = rbp.add(1).read() as *const ();
            rbp = rbp.read() as *const usize;
            if rbp.is_null() {
                break;
            }

            log::warn!("rbp: {rbp:p}; rip: {rip:p}");
        }
    }
}
