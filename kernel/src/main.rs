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
        kernel::cpu_acpi_id(),
        kernel::cpu_ncores(),
        *count,
    );
    *count += 1;
    if *count == kernel::cpu_ncores() {
        log::info!("On core {}", kernel::cpu_acpi_id());
        log::info!("All {} cores done!", kernel::cpu_ncores());

        log::info!("About to software interrupt from SUPER.");
        unsafe { asm!("int 0x80") };
        log::info!("Back!");

        log::info!("About to switch to user mode.");
        unsafe { switch_to_user_mode() };
        log::info!("In user mode!");

        log::info!("About to software interrupt from USER.");
        unsafe { asm!("int 0x80") };
        log::info!("Back!");

        log::info!("About to syscall.");
        let time = kernel::kvmclock::time(|| unsafe {
            for _ in 0..0x1000 {
                asm!("syscall");
            }
        });
        log::info!("Syscall took {:?}", time / 0x1000);

        log::info!("Shutting down.",);
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
