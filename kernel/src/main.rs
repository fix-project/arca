#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use alloc::{collections::btree_map::BTreeMap, vec::Vec};

use kernel::{rt, server};
use macros::kmain;

#[kmain]
async fn kmain() {
    log::info!("kmain");

    kernel::profile::begin();
    let server = server::SERVER.wait();
    server.run().await;
    kernel::profile::end();

    profile();
}

fn profile() {
    log::info!("most frequent functions:");
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
    for (i, &(ref name, count)) in entries.iter().take(8).enumerate() {
        log::info!("\t{i}: {count} - {name}");
    }
}
