#![no_main]
#![no_std]
#![feature(allocator_api)]

use core::time::Duration;

use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use kernel::kvmclock;
use kernel::prelude::*;
use kernel::rt;
use macros::kmain;

extern crate alloc;
extern crate kernel;

const FORCER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_forcer"));
const FORCEE: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_forcee"));

#[kmain]
async fn kmain(_: &[usize]) {
    test(100, 1 << 17, true);
    log::info!("no clone");
    let kib = 1024;
    let mib = 1024 * kib;
    let gib = 1024 * mib;
    let values = [
        4 * kib,
        16 * kib,
        64 * kib,
        256 * kib,
        mib,
        4 * mib,
        16 * mib,
        64 * mib,
        256 * mib,
    ];
    for bytes in values {
        let count = bytes / 4096;
        let iters = 100_000;
        let time_per_iter = test(iters, count, true);
        log::info!("{count} ({bytes} bytes): {time_per_iter:?}",);
    }
    log::info!("with clone");
    for bytes in values {
        let count = bytes / 4096;
        let iters = 1_000;
        let time_per_iter = test(iters, count, false);
        log::info!("{count} ({bytes} bytes): {time_per_iter:?}",);
    }
}

fn test(iters: usize, count: usize, unified: bool) -> Duration {
    let forcer = Thunk::from_elf(FORCER);
    let Value::Lambda(forcer) = forcer.run() else {
        panic!();
    };
    let forcee = Thunk::from_elf(FORCEE);
    let Value::Lambda(forcee) = forcee.run() else {
        panic!();
    };
    let forcee = forcee.apply(Value::Word(count as u64));
    let Value::Lambda(forcee) = forcee.run() else {
        panic!();
    };
    let forcee = forcee.apply(Value::Word(unified as u64));
    let forcer = forcer.apply(Value::Thunk(forcee));
    let Value::Lambda(forcer) = forcer.run() else {
        panic!();
    };
    let forcer = forcer.apply(Value::Word(iters as u64));

    let start = kvmclock::now();
    let _ = forcer.run();
    let end = kvmclock::now();
    let duration = end - start;
    let duration: Duration = Duration::from_secs_f64(duration.as_seconds_f64());
    duration / iters as u32
}

fn profile() {
    log::info!("--- MOST FREQUENT FUNCTIONS ---");
    let entries = kernel::profile::entries();
    let entries = entries
        .into_iter()
        .map(|(p, x)| {
            if p.is_null() {
                (("USER CODE".into(), 0), x)
            } else {
                (
                    kernel::host::symname(p).unwrap_or_else(|| ("???".into(), 0)),
                    x,
                )
            }
        })
        .fold(BTreeMap::new(), |mut map, ((name, _offset), count)| {
            map.entry(name).and_modify(|e| *e += count).or_insert(count);
            map
        });
    let mut entries = Vec::from_iter(entries);
    entries.sort_by_key(|(_name, count)| *count);
    entries.reverse();
    let total: usize = entries.iter().map(|(_, count)| count).sum();
    for (i, &(ref name, count)) in entries.iter().take(8).enumerate() {
        log::info!(
            "\t{i}: {count:6} ({:3.2}%)- {name}",
            count as f64 / total as f64 * 100.
        );
    }
    log::info!("-------------------------------");
}
