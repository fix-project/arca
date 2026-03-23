#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![feature(custom_test_frameworks)]
#![cfg_attr(feature = "testing-mode", test_runner(crate::testing::test_runner))]
#![cfg_attr(feature = "testing-mode", reexport_test_harness_main = "test_main")]
#![allow(dead_code)]

use kernel::prelude::*;

#[cfg(feature = "testing-mode")]
mod testing;

use fixruntime::{
    data::{BlobData, TreeData},
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
    let function = runtime.create_blob(BlobData::create(MODULE));
    log::info!("creating addend 1");
    let addend1 = runtime.create_blob_i64(7);
    log::info!("creating addend 2");
    let addend2 = runtime.create_blob_i64(1024);

    let scratch = vec![dummy, function, addend1, addend2];
    log::info!("creating combination");
    let combination = runtime.create_tree(TreeData::create(&scratch));
    log::info!("about to execute combination");
    let result = runtime.execute(&combination);
    log::info!("result is: {result:?}");
    let result_blob = runtime
        .get_blob(&result)
        .expect("Add did not return a Blob");
    let mut arr = [0u8; 8];
    result_blob.get(&mut arr);
    let num = u64::from_le_bytes(arr);
    log::info!("{:?}", num);
    assert_eq!(num, 1031);
}
