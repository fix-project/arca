#![no_std]
#![no_main]

use kernel::prelude::*;

#[kmain]
fn main(_: &[usize]) {
    log::info!("hello, world");
}
