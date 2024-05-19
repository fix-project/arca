#![no_main]
#![no_std]

extern crate kernel;

use core::sync::atomic::{AtomicUsize, Ordering};

use kernel::halt;

static PRINT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    log::info!("Hello {}!", PRINT_COUNT.fetch_add(1, Ordering::SeqCst));
    halt();
}
