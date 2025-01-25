#![no_std]
#![no_main]

extern crate user;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::exit();
}
