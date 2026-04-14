#![no_main]
#![no_std]
// #![feature(iterator_try_collect)]
// #![feature(custom_test_frameworks)]
#![cfg_attr(feature = "testing-mode", test_runner(crate::testing::test_runner))]
#![cfg_attr(feature = "testing-mode", reexport_test_harness_main = "test_main")]
#![allow(dead_code)]

use kernel::prelude::*;

#[cfg(feature = "testing-mode")]
mod testing;

use common::bitpack::BitPack;

use fixruntime::{
    fixruntime::FixRuntime,
    runtime::{DeterministicEquivRuntime, Executor},
    storage::ObjectStore,
};

extern crate alloc;

//use crate::runtime::handle;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));

#[kmain]
async fn main(_: &[usize]) {
    log::info!("creating object store");
    let mut store = ObjectStore::new();
    log::info!("creating fix runtime");
    let mut runtime = FixRuntime::new(&mut store);

    log::info!("creating resource limits");
    let dummy = runtime.create_blob_i64(0xcafeb0ba);
    log::info!("creating function");
    let function = runtime.create_blob(MODULE.into());
    log::info!("creating addend 1");
    let addend1 = runtime.create_blob_i64(7);
    log::info!("creating addend 2");
    let addend2 = runtime.create_blob_i64(1024);

    let mut scratch = Tuple::new(4);
    scratch.set(0, Blob::new(dummy.pack()));
    scratch.set(1, Blob::new(function.pack()));
    scratch.set(2, Blob::new(addend1.pack()));
    scratch.set(3, Blob::new(addend2.pack()));
    log::info!("creating combination");
    let combination = runtime.create_tree(scratch.into());
    log::info!("about to execute combination");
    let result = runtime.execute(&combination);
    log::info!("result is: {result:?}");
    let result_blob = runtime
        .get_blob(&result)
        .expect("Add did not return a Blob");
    let mut arr = [0u8; 8];
    result_blob.read(0, &mut arr);
    let num = u64::from_le_bytes(arr);
    log::info!("{:?}", num);
    assert_eq!(num, 1031);
}
