#![no_std]
#![no_main]

use kernel::{
    kthread::{self, yield_now},
    prelude::*,
};

#[kmain]
fn main(_: &[usize]) {
    log::info!("about to spawn two threads");
    kthread::spawn(|| {
        for i in 0..5 {
            log::info!("thread 1 says {i}");
            yield_now();
        }
    });
    kthread::spawn(|| {
        for i in 100..105 {
            log::info!("thread 2 says {i}");
            yield_now();
        }
    });
}
