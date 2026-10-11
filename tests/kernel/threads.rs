#![no_std]
#![no_main]

use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};
use kernel::{
    kthread::{self, KCondvar, KMutex},
    prelude::*,
};

extern crate alloc;

#[kmain]
fn main() {
    let started = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    if kernel::ncores() > 1 {
        let busy_started = started.clone();
        let busy_finished = finished.clone();
        kthread::spawn(move || {
            while kernel::coreid() != 0 {
                kthread::yield_now();
            }
            busy_started.store(true, Ordering::Release);
            while !busy_finished.load(Ordering::Acquire) {
                core::hint::spin_loop();
            }
        });
        while !started.load(Ordering::Acquire) {
            kthread::yield_now();
        }
    }
    let shared = Arc::new((KMutex::new(0), KCondvar::new()));
    for id in 0..4 {
        let shared = shared.clone();
        kthread::spawn(move || {
            for _ in 0..100 {
                let mut turn = shared.0.lock();
                while *turn % 4 != id {
                    turn = shared.1.wait(turn);
                }
                if id == 0 {
                    assert!(kernel::host::fs::exists(".").unwrap());
                }
                *turn += 1;
                kthread::yield_now();
                shared.1.notify_all();
            }
        });
    }
    let mut turn = shared.0.lock();
    while *turn != 400 {
        turn = shared.1.wait(turn);
    }
    finished.store(true, Ordering::Release);
    log::info!("threads complete");
}
