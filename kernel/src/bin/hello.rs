#![no_std]
#![no_main]

use kernel::prelude::*;
use kernel::rt;

#[kmain]
async fn main(_: &[usize]) {
    log::info!("hello, world");
}
