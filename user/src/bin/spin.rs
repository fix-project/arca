#![no_std]
#![no_main]

extern crate user;

/// Loop indefinitely.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let x: u64;
    unsafe {
        core::arch::asm!("pushf; pop {x}", x=out(reg) x);
    }

    if x & 0x200 != 0x200 {
        unsafe {
            core::arch::asm!("hlt");
        }
    }

    loop {
        core::hint::spin_loop();
    }
}
