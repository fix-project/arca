#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use core::time::Duration;

use alloc::{collections::btree_map::BTreeMap, vec::Vec};

use futures::{stream::FuturesUnordered, StreamExt};
use kernel::{
    kvmclock, rt, server,
    types::{Blob, Thunk},
};
use macros::kmain;

const INFINITE_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_infinite"));

#[kmain]
async fn kmain() {
    kernel::profile::begin();
    let set = FuturesUnordered::new();
    for _ in 0..kernel::ncores() {
        set.push(server::SERVER.wait().run());
    }
    let _: Vec<_> = set.collect().await;
    kernel::profile::end();
    profile();

    kernel::profile::reset();
    log::info!("");

    kernel::profile::begin();
    bench().await;
    kernel::profile::end();
    profile();
}

async fn bench() {
    let cores = kernel::ncores();
    let lg_cores = cores.ilog2();
    let duration = Duration::from_millis(500);
    for lg_n in 0..(lg_cores + 6) {
        let n = 1 << lg_n;
        let set = FuturesUnordered::new();
        let now = kvmclock::time_since_boot();
        for _ in 0..n {
            set.push(test(now + duration));
        }
        let results: Vec<_> = set.collect().await;
        let elapsed = kvmclock::time_since_boot() - now;
        let iters: usize = results.iter().sum();
        let time = elapsed / iters as u32;
        log::info!("{n:2} cores: {time:?} per iteration ({} total)", iters);
    }
}

async fn test(end: Duration) -> usize {
    let thunk = Thunk::from_elf(INFINITE_ELF);
    let mut iters = 0;
    while kvmclock::time_since_boot() < end {
        let thunk = thunk.clone();
        thunk.run_on_this_cpu();
        iters += 1;
    }
    iters
}

fn profile() {
    log::info!("--- MOST FREQUENT FUNCTIONS ---");
    let entries = kernel::profile::entries();
    let entries = entries
        .into_iter()
        .map(|(p, x)| {
            (
                kernel::host::symname(p).unwrap_or_else(|| ("???".into(), 0)),
                x,
            )
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
