#![no_main]
#![no_std]

extern crate kernel;

use core::sync::atomic::{AtomicUsize, Ordering};

use kernel::{halt, shutdown};

static DONE_COUNT: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    let count = DONE_COUNT.fetch_add(1, Ordering::Relaxed);
    log::info!(
        "Hello from CPU {}/{} (n={})!",
        kernel::cpu_acpi_id(),
        kernel::cpu_ncores(),
        count,
    );
    if (count + 1) == kernel::cpu_ncores() as usize {
        unsafe {
            shutdown();
        }
    }
    halt();
}
