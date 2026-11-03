#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::testing::harness)]
#![reexport_test_harness_main = "test_main"]

use kernel::prelude::*;

#[kmain]
fn main() {
    test_main();
}

#[kernel::test]
fn a_panics() {
    panic!("guest panic probe");
}

#[kernel::test]
fn b_succeeds() {}

#[kernel::test]
fn c_exits_before_completion() {
    kernel::shutdown();
}
