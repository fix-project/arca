#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![feature(alloc_layout_extra)]
#![feature(optimize_attribute)]
#![feature(const_size_of_val)]
#![feature(lazy_cell)]
#![feature(const_for)]
#![feature(const_mut_refs)]
#![feature(const_trait_impl)]
#![feature(never_type)]
#![feature(min_specialization)]
#![feature(associated_type_defaults)]
#![test_runner(crate::testing::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

#[macro_use]
pub extern crate macros;

pub use macros::core_local;

pub mod allocator;
pub mod buddy;
pub mod cpuinfo;
pub mod debugcon;
pub mod io;
pub mod kvmclock;
pub mod paging;
pub mod spinlock;
pub mod tsc;
pub mod vm;

mod gdt;
mod idt;
mod interrupts;
mod lapic;
#[allow(dead_code)]
mod msr;
mod multiboot;
mod rsstart;
mod tss;

pub use lapic::LAPIC;

#[cfg(test)]
mod testing;

pub fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}

pub fn shutdown() -> ! {
    unsafe {
        core::arch::asm!("mov cr3, {bad:r}", bad = in(reg) 0);
    }
    halt();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    shutdown()
}
