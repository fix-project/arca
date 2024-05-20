#![no_main]
#![no_std]

extern crate kernel;

use kernel::{halt, shutdown, spinlock::SpinLock};

static DONE_COUNT: SpinLock<usize> = SpinLock::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    let mut count = DONE_COUNT.lock();
    log::info!(
        "Hello from CPU {}/{} (n={})!",
        kernel::cpu_acpi_id(),
        kernel::cpu_ncores(),
        *count,
    );
    *count += 1;
    if *count == kernel::cpu_ncores() as usize {
        log::info!("All {} cores done!", kernel::cpu_ncores());
        unsafe {
            shutdown();
        }
    }
    count.unlock();
    halt();
}
