#![no_std]
#![no_main]

extern crate user;

/// Accepts an infinite number of arguments without ever producing a value.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        let _ = user::os::prompt();
    }
}
