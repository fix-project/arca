#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

pub mod debugcon;
pub mod io;

mod rsstart;

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
