#![no_main]
#![no_std]

mod rsstart;

use core::{
    arch::asm,
    fmt::Write,
    panic::PanicInfo,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value);
}

unsafe fn shutdown() {
    asm!("mov cr3, {bad:r}", bad = in(reg) 0xffffffffffffffffu64);
}

pub struct DebugConsole();

static CONSOLE_LOCK: AtomicBool = AtomicBool::new(false);

impl core::fmt::Write for DebugConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            unsafe {
                outb(0xe9, c);
            }
        }
        Ok(())
    }
}

static PRINT_COUNT: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
#[inline(never)]
extern "C" fn kmain(id: u32, boot: bool, ncores: u32, _multiboot: *const ()) -> ! {
    let mut s = DebugConsole();
    let cpu_type = if boot { "BP" } else { "AP" };
    while CONSOLE_LOCK
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        unsafe {
            asm!("pause");
        }
    }
    let _ = writeln!(s, "Hello from {cpu_type} {id} of {ncores}!");
    CONSOLE_LOCK.store(false, Ordering::SeqCst);
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
fn panic(_: &PanicInfo) -> ! {
    loop {
        unsafe {
            asm!("pause");
        }
    }
}
