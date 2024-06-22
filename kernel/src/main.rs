#![no_main]
#![no_std]

extern crate alloc;

extern crate kernel;

use core::arch::asm;

use kernel::{halt, shutdown, spinlock::SpinLock};

static DONE_COUNT: SpinLock<usize> = SpinLock::new(0);

extern "C" {
    fn switch_to_user_mode();
}

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
        kernel::cpuinfo::id(),
        kernel::cpuinfo::ncores(),
        *count,
    );
    *count += 1;
    if *count == kernel::cpuinfo::ncores() {
        log::info!("On core {}", kernel::cpuinfo::id());
        log::info!("All {} cores done!", kernel::cpuinfo::ncores());

        log::info!("About to software interrupt from SUPER.");
        unsafe { asm!("int 0x80") };
        log::info!("Back!");

        log::info!("About to switch to user mode.");
        unsafe { switch_to_user_mode() };
        log::info!("In user mode!");

        log::info!("About to software interrupt from USER.");
        let iters = 0x1000;
        let time = kernel::tsc::time(|| unsafe {
            for _ in 0..iters {
                asm!("int 0x80");
            }
        });
        log::info!("Software Interrupt took {:?}", time / iters);

        log::info!("About to syscall from USER.");
        let iters = 0x1000;
        let time = kernel::tsc::time(|| unsafe {
            for _ in 0..iters {
                asm!("syscall", out("rcx")_, out("r11")_);
            }
        });
        log::info!("Syscall took {:?}", time / iters);

        log::info!("Shutting down.");
        unsafe {
            shutdown();
        }
    }
    count.unlock();
    halt();
}

#[no_mangle]
unsafe extern "C" fn syscall() {
    log::info!("hello");
}
