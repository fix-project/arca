#![no_std]
#![no_main]

extern crate user;

/// Return the function's argument unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(1);
    user::syscall::prompt(0);
    user::syscall::exit(0);
}
