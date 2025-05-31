#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Try to corrupt kernel data, which should fail.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let result = unsafe { syscall(defs::syscall::SYS_CREATE_BLOB, 0xff, 8) };
    if result >= 0 {
        unsafe {
            core::arch::asm!("int3");
        }
    }
    os::exit(os::null());
}
