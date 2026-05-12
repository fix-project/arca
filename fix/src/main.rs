#![no_main]
#![no_std]
// #![feature(iterator_try_collect)]
// #![feature(custom_test_frameworks)]
#![cfg_attr(feature = "testing-mode", test_runner(crate::testing::test_runner))]
#![cfg_attr(feature = "testing-mode", reexport_test_harness_main = "test_main")]
#![allow(dead_code)]

use fixhandle::rawhandle::{Encode, FixHandle, Thunk};
use kernel::prelude::*;

#[cfg(feature = "testing-mode")]
mod testing;

mod evaluator;

use common::bitpack::BitPack;

use fixruntime::{
    fixruntime::FixRuntime, runtime::DeterministicEquivRuntime, storage::ObjectStore,
};

use crate::evaluator::{eval, get_coupon_rhs, show_coupon};

extern crate alloc;

//use crate::runtime::handle;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));
const COUPON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/coupon"));

#[kmain]
async fn main(_: &[usize]) {
    let mut store = ObjectStore::new();
    let mut runtime = FixRuntime::new(&mut store, COUPON);

    log::info!("runnning + (+ 3 4) 1024");
    log::info!("creating resource limits");
    let dummy = runtime.create_blob_i64(0xcafeb0ba);
    log::info!("creating function");
    let function = runtime.create_blob(MODULE.into());
    log::info!("creating addend 3");
    let addend1 = runtime.create_blob_i64(3);
    log::info!("creating addend 4");
    let addend2 = runtime.create_blob_i64(4);
    log::info!("creating addend 1024");
    let addend3 = runtime.create_blob_i64(1024);

    let mut scratch = Tuple::new(4);
    scratch.set(0, Blob::new(dummy.pack()));
    scratch.set(1, Blob::new(function.pack()));
    scratch.set(2, Blob::new(addend1.pack()));
    scratch.set(3, Blob::new(addend2.pack()));
    let combination = runtime.create_tree(scratch);
    let application = FixHandle::Thunk(Thunk::Application(
        combination.unwrap_object().unwrap_tree_obj(),
    ));
    let encode = FixHandle::Encode(Encode::Strict(application.unwrap_thunk()));

    let mut scratch = Tuple::new(4);
    scratch.set(0, Blob::new(dummy.pack()));
    scratch.set(1, Blob::new(function.pack()));
    scratch.set(2, Blob::new(encode.pack()));
    scratch.set(3, Blob::new(addend3.pack()));
    let combination = runtime.create_tree(scratch);
    let application = FixHandle::Thunk(Thunk::Application(
        combination.unwrap_object().unwrap_tree_obj(),
    ));
    let encode = FixHandle::Encode(Encode::Strict(application.unwrap_thunk()));

    let eval_coupon = eval(&mut runtime, encode);

    show_coupon(&mut runtime, &eval_coupon);

    let result_blob = get_coupon_rhs(&mut runtime, &eval_coupon);
    let result_blob = runtime
        .get_blob(&result_blob)
        .expect("Result is not a Blob");
    let mut arr = [0u8; 8];
    result_blob.read(0, &mut arr);
    let num = u64::from_le_bytes(arr);
    log::info!("{:?}", num);
    assert_eq!(num, 1031);
}
