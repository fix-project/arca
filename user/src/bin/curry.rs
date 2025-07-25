#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Takes a function which requires an n-tuple and the number n, and returns an n-ary function
/// which evaluates to the same thing.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let f: Function = os::argument()
        .try_into()
        .expect("first argument should be a function");
    let n: Word = os::argument()
        .try_into()
        .expect("second argument should be a word");
    let n = n.read() as usize;

    let x: Tuple = (0..n).map(|_| os::argument()).collect();

    let y = f(x);
    os::exit(y);
}
