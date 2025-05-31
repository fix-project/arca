#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let argument = os::prompt();
    let DynValue::Tree(mut tree) = argument.into() else {
        panic!("incorrect argument type to add");
    };
    assert_eq!(tree.len(), 2);
    let DynValue::Word(x) = tree.take(0).into() else {
        panic!("incorrect argument type to add");
    };
    let DynValue::Word(y) = tree.take(1).into() else {
        panic!("incorrect argument type to add");
    };

    let z = x.read() + y.read();

    os::exit(os::word(z));
}
