#![no_std]
#![no_main]

use user::prelude::*;

extern crate user;

/// Return the function's argument unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let argument = os::argument();
    os::exit(argument);
}
