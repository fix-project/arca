#![no_main]
#![no_std]

extern crate kernel;

use core::{
    arch::asm,
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};

unsafe fn shutdown() {
    asm!("mov cr3, {bad:r}", bad = in(reg) 0xffffffffffffffffu64);
}

static PRINT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain(id: u32, boot: bool, ncores: u32, _multiboot: *const ()) -> ! {
    let cpu_type = if boot { "BP" } else { "AP" };
    log::info!("Hello from {cpu_type} {id} of {ncores}!");
    let old = PRINT_COUNT.fetch_add(1, Ordering::SeqCst) as u32;
    if old == ncores - 1 {
        unsafe {
            shutdown();
        }
    }
    loop {
        unsafe {
            asm!("pause");
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{}", info);
    unsafe {
        loop {
            asm!("pause");
        }
    }
}
