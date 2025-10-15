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

use crate::{handle::FixRuntime, runtime::DeterministicEquivRuntime, runtime::ExecutionRuntime};

extern crate alloc;

pub mod handle;
pub mod runtime;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));

#[kmain]
async fn main(_: &[usize]) {
    let dummy = FixRuntime::create_blob_i64(0xcafeb0ba);
    let function = FixRuntime::create_blob(Value::Blob(Runtime::create_blob(MODULE)));

    let mut tree = FixRuntime::create_scrach_tree(4);
    let _ = FixRuntime::set_tree_entry(&mut tree, 0, &dummy);
    let _ = FixRuntime::set_tree_entry(&mut tree, 1, &function);
    let _ = FixRuntime::set_tree_entry(&mut tree, 2, &FixRuntime::create_blob_i64(7));
    let _ = FixRuntime::set_tree_entry(&mut tree, 3, &FixRuntime::create_blob_i64(1024));
    let combination = FixRuntime::create_tree(tree);
    let result = FixRuntime::execute(&combination).expect("Failed to execute");

    let mut arr = [0u8; 8];
    let result_blob = FixRuntime::get_blob(&result).expect("Add did not return a Blob");
    arr[..result_blob.len()].copy_from_slice(result_blob);
    let num = u64::from_le_bytes(arr);
    log::info!("{:?}", num);
    assert_eq!(num, 1031);
}
