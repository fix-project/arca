#![no_std]
#![no_main]

extern crate user;

/// Accepts an infinite number of arguments without ever producing a value.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(1);
    loop {
        user::syscall::prompt(0);
    }
}
