#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use core::time::Duration;

use alloc::{collections::btree_map::BTreeMap, vec, vec::Vec};

use kernel::{
    kvmclock, rt, server,
    types::{Thunk, Value},
};
use macros::kmain;

const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));

#[kmain]
async fn kmain() {
    // kernel::profile::begin();
    server::SERVER.wait().run().await;
    // kernel::profile::end();
    // profile();

    kernel::profile::reset();
    // log::info!("");

    kernel::profile::begin();
    bench().await;
    kernel::profile::end();
    profile();
}

async fn bench() {
    let cores = kernel::ncores();
    let lg_cores = cores.ilog2();
    let duration = Duration::from_millis(1000);
    let options = 0..(lg_cores + 3);
    for lg_n in options {
        let n = 1 << lg_n;
        log::info!("");
        log::info!("*** running on {n} threads ***");
        let mut set = Vec::with_capacity(n);
        let now = kvmclock::time_since_boot();
        rt::reset_stats();
        for _ in 0..n {
            set.push(rt::spawn(test(now + duration)));
        }
        let mut results = vec![];
        for x in set {
            results.push(x.await);
        }
        rt::profile();
        let elapsed = kvmclock::time_since_boot() - now;
        let iters: usize = results.iter().sum();
        let time = elapsed / iters as u32;
        let iters_per_second = iters as f64 / elapsed.as_secs_f64();
        log::info!("{n:4}/{cores:<3} threads: {time:?} per iteration ({iters_per_second:.2} iters/second) - ran for ({elapsed:?})",);
    }
}

async fn test(end: Duration) -> usize {
    let thunk = Thunk::from_elf(ADD_ELF);
    let Value::Lambda(lambda) = thunk.run() else {
        panic!();
    };
    let mut iters = 0;
    while kvmclock::time_since_boot() < end {
        let lambda = lambda.clone();
        let thunk = lambda.apply(Value::Tree(vec![Value::Word(1), Value::Word(2)].into()));
        let _ = thunk.run();
        iters += 1;
        if iters % 1000 == 0 {
            rt::yield_now().await;
        }
    }
    iters
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
