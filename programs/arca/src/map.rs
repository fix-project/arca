#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Map a Function over a Tuple.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let f: Function = os::argument()
        .try_into()
        .expect("incorrect argument type to map");
    let xs: Tuple = os::argument()
        .try_into()
        .expect("incorrect argument type to map");

    let ys: Tuple = xs.into_iter().map(|x| f.clone()(x)).collect();

    os::exit(ys);
}
