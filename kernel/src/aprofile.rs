use core::{
    fmt::Write as _,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

pub use macros::profile;

use alloc::collections::btree_map::BTreeMap;

use crate::{debugcon, prelude::*};

static COUNTS: OnceLock<&'static [AtomicU64]> = OnceLock::new();

extern "C" {
    static mut _srodata: u8;
    static mut _edata: u8;
}

pub(crate) unsafe fn init() {
    COUNTS.get_or_init(|| {
        let size = (&raw const _edata).offset_from_unsigned(&raw const _srodata);
        let buffer = Box::new_zeroed_slice(size).assume_init();
        Box::leak(buffer)
    });
}

pub fn reset() {
    for count in *COUNTS {
        count.store(0, Ordering::SeqCst);
    }
}

pub fn log_time_spent(f: &&'static str, duration: Duration) {
    let ns = duration.as_nanos() as u64;
    let start = &raw const _srodata;
    let end = &raw const _edata;
    let rip = f as *const &'static str as *const u8;
    if rip < start || rip >= end {
        return;
    }
    unsafe {
        let offset = rip.offset_from_unsigned(start);
        COUNTS[offset].fetch_add(ns, Ordering::SeqCst);
    }
}

pub fn entries() -> BTreeMap<&'static str, Duration> {
    let start = &raw const _srodata as *const ();

    let mut entries = BTreeMap::new();
    for (i, count) in COUNTS.iter().enumerate() {
        let count = count.load(Ordering::SeqCst);

        if count > 0 {
            unsafe {
                entries.insert(
                    *(start.byte_add(i) as *const &'static str),
                    Duration::from_nanos(count),
                );
            }
        }
    }
    entries
}

pub fn report(entries: &mut [(&'static str, Duration)]) {
    let start = &raw const _srodata;
    static NULL: &str = "N/A";
    entries.fill((NULL, Duration::ZERO));

    for (i, count) in COUNTS.iter().enumerate() {
        let count = count.load(Ordering::SeqCst);
        let dur = Duration::from_nanos(count);

        if dur > entries[0].1 {
            unsafe {
                entries[0] = (*(start.byte_add(i) as *const &'static str), dur);
            }
        }

        entries.sort_by_key(|x| x.1);
    }
    entries.reverse();
}

pub fn log(count: usize) {
    let mut entries = vec![("", Duration::ZERO); count];
    report(&mut entries);
    let mut console = debugcon::CONSOLE.lock();
    let _ = writeln!(&mut *console, "----- PROFILE -----");
    for (i, (p, n)) in entries.iter().enumerate() {
        if *n == Duration::ZERO {
            break;
        }
        let symname = *p;
        let n = n.as_nanos();
        let _ = writeln!(&mut *console, "{i}. {n:.5e}\t{symname}");
    }
    let _ = writeln!(&mut *console, "-------------------");
}
