#![no_std]
#![allow(dead_code)]
#![cfg_attr(feature = "testing-mode", feature(custom_test_frameworks))]
#![cfg_attr(feature = "testing-mode", test_runner(crate::testing::test_runner))]
#![cfg_attr(feature = "testing-mode", reexport_test_harness_main = "test_main")]

#[cfg(feature = "testing-mode")]
mod testing;

pub mod rawhandle;
