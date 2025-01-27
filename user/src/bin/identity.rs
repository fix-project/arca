#![no_std]
#![no_main]

extern crate user;

/// Return the function's argument unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let arg = 0;
    user::syscall::resize(1);
    user::syscall::argument(arg);
    user::syscall::exit(arg);
}
