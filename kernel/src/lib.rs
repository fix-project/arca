#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![feature(never_type)]
#![feature(negative_impls)]
#![feature(allocator_api)]
#![feature(box_as_ptr)]
#![feature(bigint_helper_methods)]
#![feature(box_into_inner)]
#![feature(new_zeroed_alloc)]
#![feature(ptr_metadata)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;
extern crate defs;

#[macro_use]
pub extern crate macros;

pub use macros::core_local;

pub mod allocator;
pub mod cpu;
pub mod debugcon;
pub mod host;
pub mod io;
pub mod kvmclock;
pub mod page;
pub mod paging;
pub mod prelude;
pub mod profile;
pub mod rt;
pub mod server;
pub mod tsc;
pub mod types;
pub mod vm;

mod gdt;
mod idt;
mod interrupts;
mod lapic;
mod msr;
mod registers;
mod rsstart;
mod tss;

pub use common::util::initcell;
pub use common::util::spinlock;
pub use lapic::LAPIC;

use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
mod testing;

#[no_mangle]
static mut EXIT_CODE: u8 = 0;

pub(crate) static NCORES: AtomicUsize = AtomicUsize::new(0);

pub fn coreid() -> u32 {
    let mut id: u32 = 0;
    unsafe {
        core::arch::x86_64::__rdtscp(&mut id);
    }
    id
}

pub fn ncores() -> usize {
    NCORES.load(Ordering::SeqCst)
}

pub fn halt() {
    unsafe {
        core::arch::asm!("hlt");
    }
}

pub fn pause() {
    unsafe {
        core::arch::x86_64::_mm_pause();
    }
}

pub fn shutdown() -> ! {
    exit(0);
}

pub fn exit(code: u8) -> ! {
    loop {
        unsafe {
            io::outb(0, code);
        }
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use core::fmt::Write;
    use spinlock::SpinLockGuard;

    let mut console = crate::debugcon::CONSOLE.lock();
    let _ = writeln!(&mut *console, "KERNEL PANIC: {}", info);
    SpinLockGuard::unlock(console);

    loop {
        unsafe { io::outb(0, 1) }
        core::hint::spin_loop();
    }
}
