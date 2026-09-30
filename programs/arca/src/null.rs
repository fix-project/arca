#![no_std]
#![no_main]

use user::prelude::*;

extern crate user;

/// Return a null value.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    os::exit(Null::new());
}
