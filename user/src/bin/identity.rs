#![no_std]
#![no_main]

extern crate user;

/// Return the function's argument unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let arg = user::syscall::argument();
    user::syscall::exit(arg);
}
