#![no_main]
#![no_std]

extern crate alloc;
extern crate kernel;

use kernel::{halt, shutdown, spinlock::SpinLock};

static DONE_COUNT: SpinLock<usize> = SpinLock::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    if cfg!(test) {
        unsafe {
            shutdown();
        }
    }

    let mut count = DONE_COUNT.lock();
    log::info!(
        "Hello from CPU {}/{} (n={})!",
        kernel::cpu_acpi_id(),
        kernel::cpu_ncores(),
        *count,
    );
    *count += 1;
    if *count == kernel::cpu_ncores() {
        log::info!("All {} cores done!", kernel::cpu_ncores());
        log::info!(
            "Shutting down at {} after {:?}.",
            kernel::kvmclock::wall_clock_time(),
            kernel::kvmclock::time_since_boot()
        );
        let iters = 100_000;
        let f = || {
            for _ in 0..iters {
                kernel::tsc::read_cycles();
            }
        };
        let total = kernel::kvmclock::time(f) / iters;
        log::info!("KVMClock: calling function took {total:?}.",);
        let total = kernel::tsc::time(f) / iters;
        log::info!("TSC: calling function took {total:?}.",);
        unsafe {
            shutdown();
        }
    }
    count.unlock();
    halt();
}
