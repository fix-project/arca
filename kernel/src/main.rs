#![no_main]
#![no_std]

extern crate kernel;

use core::sync::atomic::{AtomicUsize, Ordering};

static PRINT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain(id: u32, boot: bool, ncores: u32) -> ! {
    let cpu_type = if boot { "BP" } else { "AP" };
    log::info!("Hello from {cpu_type} {id} of {ncores}!");
    let old = PRINT_COUNT.fetch_add(1, Ordering::SeqCst) as u32;
    if old == ncores - 1 {
        unsafe { kernel::shutdown() }
    }
    loop {
        unsafe { core::arch::x86_64::_mm_pause() }
    }
}
