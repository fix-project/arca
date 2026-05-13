#![no_main]
#![no_std]
// #![feature(iterator_try_collect)]
// #![feature(custom_test_frameworks)]
#![cfg_attr(feature = "testing-mode", test_runner(crate::testing::test_runner))]
#![cfg_attr(feature = "testing-mode", reexport_test_harness_main = "test_main")]
#![allow(dead_code)]

use fixhandle::rawhandle::FixHandle;
use fixruntime::{
    common::{CouponTrades, FixOp},
    runtime::{CouponHelper, DeterministicEquivRuntime, Executor},
};
use kernel::prelude::*;

#[cfg(feature = "testing-mode")]
mod testing;

mod evaluator;

use common::bitpack::BitPack;

use fixruntime::{fixruntime::FixRuntime, storage::ObjectStore};

use crate::evaluator::eval;

extern crate alloc;

//use crate::runtime::handle;

const MODULE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/addblob"));
const COUPON: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/coupon"));

#[kmain]
async fn main(args: &[usize]) {
    let args: &[usize; 8] = args.try_into().unwrap();
    let opcode = FixOp::try_from(args[0]).expect("Failed to parse opcode");
    let (handle_scratch_offset, handle_scratch_len) = (args[2], args[3]);
    let (store_offset, store_len) = (args[4], args[5]);
    let (output_store_offset, output_store_len) = (args[6], args[7]);

    let mut handle_scratch: Box<[u8]> =
        ObjectStore::from_raw_parts(handle_scratch_offset, handle_scratch_len);
    let handle_slice: &mut [u8; 32] = handle_scratch.as_mut().try_into().unwrap();
    let input_handle = FixHandle::unpack(*handle_slice);

    let input_store: Box<[(usize, usize)]> = ObjectStore::from_raw_parts(store_offset, store_len);
    let mut output_store: Box<[usize]> =
        ObjectStore::from_raw_parts(output_store_offset, output_store_len);

    let mut store = ObjectStore::new();
    store.load(input_store);
    let mut runtime = FixRuntime::new(&mut store, COUPON);

    let result = match opcode {
        FixOp::Eval => eval(&mut runtime, input_handle),
        FixOp::Apply => runtime.execute(&input_handle),
        FixOp::Trade => {
            let trade_type = CouponTrades::try_from(args[1]).expect("Failed to parse coupon trade");

            let input_tree = runtime
                .get_tree(&input_handle)
                .expect("Input handle is not a tree");
            let coupons = FixRuntime::<'_>::get_tree_entry(&input_tree, 0);
            let lhs = FixRuntime::<'_>::get_tree_entry(&input_tree, 1);
            let rhs = FixRuntime::<'_>::get_tree_entry(&input_tree, 2);

            runtime.trade(trade_type, coupons, lhs, rhs)
        }
    };

    let output_store_slice: &mut [usize; 2] = output_store
        .as_mut()
        .try_into()
        .expect("Failed to convert output store back");
    let (output_store_offset, output_store_len) = ObjectStore::into_raw_parts(store.unload());
    output_store_slice[0] = output_store_offset;
    output_store_slice[1] = output_store_len;
    handle_slice.copy_from_slice(&result.pack());
}
