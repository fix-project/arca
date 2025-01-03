#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![feature(alloc_layout_extra)]
#![feature(optimize_attribute)]
#![feature(never_type)]
#![feature(maybe_uninit_slice)]
#![feature(maybe_uninit_write_slice)]
#![feature(maybe_uninit_uninit_array_transpose)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![feature(negative_impls)]
#![feature(slice_from_ptr_range)]
#![feature(new_zeroed_alloc)]
#![feature(allocator_api)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

#[macro_use]
pub extern crate macros;

use core::cell::OnceCell;

pub use macros::core_local;

pub mod allocator;
pub mod arca;
pub mod buddy;
pub mod cpu;
pub mod cpuinfo;
pub mod debugcon;
pub mod io;
pub mod kvmclock;
pub mod page;
pub mod paging;
pub mod refcnt;
pub mod rt;
pub mod spinlock;
pub mod tsc;
pub mod vm;

mod acpi;
mod arrayvec;
mod gdt;
mod idt;
mod interrupts;
mod lapic;
#[allow(dead_code)]
mod msr;
mod multiboot;
mod registers;
mod rsstart;
mod tss;

pub use lapic::LAPIC;
use spinlock::SpinLock;

#[cfg(test)]
mod testing;

pub(crate) static PHYSICAL_ALLOCATOR: SpinLock<OnceCell<common::BuddyAllocator>> =
    SpinLock::new(OnceCell::new());

pub fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}

pub fn pause() {
    unsafe { core::arch::asm!("pause") }
}

pub fn shutdown() -> ! {
    unsafe {
        // it's very unlikely the zero page contains a valid page table
        core::arch::asm!("mov cr3, {bad:r}", bad = in(reg) 0);
        // since the kernel is marked as global we need to manually invalidate the relevant TLB
        // entry
        core::arch::asm!("invlpg [{addr}]", addr=in(reg) &shutdown);
        // we need the invalidation to go through before executing any future instructions
        core::arch::asm!("serialize");
    }
    halt();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    unsafe {
        io::outw(0xf4, 1);
    }
    shutdown()
}
