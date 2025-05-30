#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Performs a side effect and returns the result of the side effect unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let effect = "effect";
    let x = effect.as_bytes();
    let atom = os::atom(x);
    let result = os::perform(atom);
    os::exit(result);
}
