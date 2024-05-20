#![no_main]
#![no_std]
#![feature(thread_local)]
#![feature(custom_test_frameworks)]
#![feature(alloc_layout_extra)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

pub mod debugcon;
pub mod io;
pub mod spinlock;
pub mod vm;

mod allocator;
mod buddy;
mod multiboot;
mod rsstart;

#[thread_local]
static mut CPU_ACPI_ID: u32 = 0;

#[thread_local]
static mut CPU_IS_BOOTSTRAP: bool = false;

#[thread_local]
static mut CPU_NCORES: u32 = 0;

pub fn cpu_acpi_id() -> u32 {
    unsafe { CPU_ACPI_ID }
}

pub fn cpu_is_bootstrap() -> bool {
    unsafe { CPU_IS_BOOTSTRAP }
}

pub fn cpu_ncores() -> u32 {
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
    for _ in 0..0x100000 {
        core::arch::asm!("pause");
    }
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
