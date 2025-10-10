#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![allow(dead_code)]

use arca::Runtime;
use kernel::prelude::*;

extern crate alloc;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));

#[kmain]
async fn main(_: &[usize]) {
    let f = common::elfloader::load_elf(MODULE).expect("Failed to load elf");
    let mut tree = Runtime::create_tuple(4);
    let dummy = Runtime::create_word(0xcafeb0ba);

    tree.set(0, dummy);
    tree.set(1, dummy);
    tree.set(2, Runtime::create_word(7));
    tree.set(3, Runtime::create_word(1024));

    let f = Runtime::apply_function(f, arca::Value::Tuple(tree));
    let word: Word = f.force().try_into().unwrap();
    log::info!("{:?}", word.read());
    assert_eq!(word.read(), 1031);
}
