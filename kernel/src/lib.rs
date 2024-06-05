#![no_main]
#![no_std]
#![feature(thread_local)]
#![feature(custom_test_frameworks)]
#![feature(alloc_layout_extra)]
#![feature(optimize_attribute)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod allocator;
pub mod buddy;
pub mod debugcon;
pub mod io;
pub mod kvmclock;
pub mod spinlock;
pub mod tsc;
pub mod vm;

mod multiboot;
mod rsstart;

#[thread_local]
static mut CPU_ACPI_ID: usize = 0;

#[thread_local]
static mut CPU_IS_BOOTSTRAP: bool = false;

#[thread_local]
static mut CPU_NCORES: usize = 0;

pub fn cpu_acpi_id() -> usize {
    unsafe { CPU_ACPI_ID }
}

pub fn cpu_is_bootstrap() -> bool {
    unsafe { CPU_IS_BOOTSTRAP }
}

pub fn cpu_ncores() -> usize {
    unsafe { CPU_NCORES }
}

pub fn halt() -> ! {
    loop {
        unsafe { core::arch::asm!("hlt") }
    }
}

/// # Safety
/// This function triggers a complete shutdown of the processor.
pub unsafe fn shutdown() -> ! {
    core::arch::asm!("mov cr3, {bad:r}", bad = in(reg) 0xffffffffffffffffu64);
    halt();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("{}", info);
    unsafe { shutdown() }
}

pub fn test_runner(tests: &[&dyn Fn()]) {
    log::info!("Running {} tests", tests.len());
    for test in tests {
        test();
    }
}

#[cfg(test)]
#[no_mangle]
extern "C" fn kmain(_: u32, bsp: bool, _: u32, _: *const ()) -> ! {
    if bsp {
        test_main();
    }
    unsafe { shutdown() }
}
