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

use common::bitpack::BitPack;

use fixruntime::{
    fixruntime::FixRuntime,
    runtime::{CouponCollector, DeterministicEquivRuntime, Executor},
    storage::ObjectStore,
};

extern crate alloc;

//use crate::runtime::handle;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));
const COUPON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/coupon"));

fn coupon_type(runtime: &mut FixRuntime, handle: &FixHandle) -> &'static str {
    let mut arr = [0u8; 4];
    let blob = runtime.get_blob(handle).expect("Coupon type not a blob");
    blob.read(0, &mut arr);
    let num = u32::from_le_bytes(arr);
    match num {
        0 => "Eq",
        1 => "Eval",
        2 => "Apply",
        3 => "Force",
        4 => "Think",
        5 => "Storage",
        _ => panic!(),
    }
}

fn get_coupon_lhs(coupon_content: &Tuple) -> FixHandle {
    let mut handle_scratch: [u8; 32] = [0; 32];
    let entry: Blob = coupon_content
        .get(2)
        .try_into()
        .expect("author not a handle");
    entry.read(0, &mut handle_scratch);
    FixHandle::unpack(handle_scratch)
}

fn get_coupon_lhs_h(runtime: &mut FixRuntime, coupon: &FixHandle) -> FixHandle {
    let coupon_content = runtime.get_tree(coupon).expect("Coupon not a tree");
    get_coupon_lhs(&coupon_content)
}

fn get_coupon_rhs(coupon_content: &Tuple) -> FixHandle {
    let mut handle_scratch: [u8; 32] = [0; 32];
    let entry: Blob = coupon_content
        .get(3)
        .try_into()
        .expect("author not a handle");
    entry.read(0, &mut handle_scratch);
    FixHandle::unpack(handle_scratch)
}

fn get_coupon_rhs_h(runtime: &mut FixRuntime, coupon: &FixHandle) -> FixHandle {
    let coupon_content = runtime.get_tree(coupon).expect("Coupon not a tree");
    get_coupon_rhs(&coupon_content)
}

fn show_coupon(runtime: &mut FixRuntime, handle: &FixHandle) {
    let coupon_content = runtime.get_tree(handle).expect("Coupon not a tree");

    let entry: Blob = coupon_content
        .get(1)
        .try_into()
        .expect("author not a handle");
    let mut handle_scratch: [u8; 32] = [0; 32];
    entry.read(0, &mut handle_scratch);
    let handle = FixHandle::unpack(handle_scratch);

    let ctype = coupon_type(runtime, &handle);
    let lhs = get_coupon_lhs(&coupon_content);
    let rhs = get_coupon_rhs(&coupon_content);

    log::info!("type is: {ctype:?}");
    log::info!("lhs is: {lhs:?}");
    log::info!("rhs is: {rhs:?}");
}

#[kmain]
async fn main(_: &[usize]) {
    log::info!("creating object store");
    let mut store = ObjectStore::new();
    log::info!("creating fix runtime");
    let mut runtime = FixRuntime::new(&mut store, COUPON);

    log::info!("creating resource limits");
    let dummy = runtime.create_blob_i64(0xcafeb0ba);
    log::info!("creating function");
    let function = runtime.create_blob(MODULE.into());
    log::info!("creating addend 7");
    let addend1 = runtime.create_blob_i64(7);
    log::info!("creating addend 1024");
    let addend2 = runtime.create_blob_i64(1024);

    let mut scratch = Tuple::new(4);
    scratch.set(0, Blob::new(dummy.pack()));
    scratch.set(1, Blob::new(function.pack()));
    scratch.set(2, Blob::new(addend1.pack()));
    scratch.set(3, Blob::new(addend2.pack()));
    let combination = runtime.create_tree(scratch);
    let apply_coupon = runtime.execute(&combination);
    let application = FixHandle::Thunk(Thunk::Application(
        combination.unwrap_object().unwrap_tree_obj(),
    ));

    let coupons = runtime.create_tree(Tuple::new(0));
    let eval_dummy = runtime.trade(
        fixruntime::runtime::CouponTrades::EvalBlobObj,
        coupons,
        dummy,
        dummy,
    );
    let eval_function = runtime.trade(
        fixruntime::runtime::CouponTrades::EvalBlobObj,
        coupons,
        function,
        function,
    );
    let eval_addend1 = runtime.trade(
        fixruntime::runtime::CouponTrades::EvalBlobObj,
        coupons,
        addend1,
        addend1,
    );
    let eval_addend2 = runtime.trade(
        fixruntime::runtime::CouponTrades::EvalBlobObj,
        coupons,
        addend2,
        addend2,
    );

    let mut coupons = Tuple::new(4);
    coupons.set(0, Blob::new(eval_dummy.pack()));
    coupons.set(1, Blob::new(eval_function.pack()));
    coupons.set(2, Blob::new(eval_addend1.pack()));
    coupons.set(3, Blob::new(eval_addend2.pack()));
    let coupons = runtime.create_tree(coupons);

    let eval_coupon = runtime.trade(
        fixruntime::runtime::CouponTrades::EvalTreeObj,
        coupons,
        combination,
        combination,
    );

    let mut coupons = Tuple::new(2);
    coupons.set(0, Blob::new(eval_coupon.pack()));
    coupons.set(1, Blob::new(apply_coupon.pack()));
    let coupons = runtime.create_tree(coupons);
    let rhs = get_coupon_rhs_h(&mut runtime, &apply_coupon);
    let think_coupon = runtime.trade(
        fixruntime::runtime::CouponTrades::ThinkApplication,
        coupons,
        application,
        rhs,
    );

    let mut coupons = Tuple::new(1);
    coupons.set(0, Blob::new(think_coupon.pack()));
    let coupons = runtime.create_tree(coupons);
    let force_coupon = runtime.trade(
        fixruntime::runtime::CouponTrades::ThinkToForce,
        coupons,
        application,
        rhs,
    );

    let encode = FixHandle::Encode(Encode::Strict(application.unwrap_thunk()));
    let mut coupons = Tuple::new(1);
    coupons.set(0, Blob::new(force_coupon.pack()));
    let coupons = runtime.create_tree(coupons);
    let eq_coupon = runtime.trade(
        fixruntime::runtime::CouponTrades::ForceToEncodeStric,
        coupons,
        encode,
        rhs,
    );

    show_coupon(&mut runtime, &eq_coupon);

    let result_blob = get_coupon_rhs_h(&mut runtime, &eq_coupon);
    let result_blob = runtime
        .get_blob(&result_blob)
        .expect("Result is not a Blob");
    let mut arr = [0u8; 8];
    result_blob.read(0, &mut arr);
    let num = u64::from_le_bytes(arr);
    log::info!("{:?}", num);
    assert_eq!(num, 1031);
    //
    //let mut result_handle: [u8; 32] = [0; 32];
    //let result_blob: Blob = runtime
    //    .get_tree(&apply_coupon)
    //    .expect("Add did not return a Tree")
    //    .get(3)
    //    .try_into()
    //    .expect("The result entry is not an Arca Blob");
    //result_blob.read(0, &mut result_handle);
    //let result_handle = FixHandle::unpack(result_handle);
    //log::info!("result handle is: {result_handle:?}");
    //let result_blob = runtime
    //    .get_blob(&result_handle)
    //    .expect("The result is not a Blob");
}
