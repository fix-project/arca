#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;
extern crate kernel;

use alloc::{collections::btree_map::BTreeMap, vec::Vec};

use kernel::{rt, types::Thunk};
use macros::kmain;

const MMAP_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_mmap"));

#[kmain]
async fn kmain(_: &[usize]) {
    kernel::profile::begin();
    let thunk = Thunk::from_elf(MMAP_ELF);
    log::info!("{:x?}", thunk.run());
    kernel::profile::end();
    profile();
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
