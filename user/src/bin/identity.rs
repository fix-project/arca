#![no_std]
#![no_main]

use user::prelude::*;

extern crate user;

/// Return the function's argument unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let argument = os::prompt();
    os::exit(argument);
}
