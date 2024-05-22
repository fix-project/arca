#![no_main]
#![no_std]

extern crate alloc;
extern crate kernel;

use core::arch::asm;

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
        let iters = 100000;
        let mut total = 0;
        total += kernel::kvmclock::time(|| {
            for _ in 0..iters {
                let mut z: u64 = 0;
                unsafe { asm!("add {z}, 10", z=inout(reg)z) };
                let _ = z;
            }
        });
        total /= iters;
        log::info!("Test: calling add function took {total} cycles.",);
        unsafe {
            shutdown();
        }
    }
    count.unlock();
    halt();
}
