#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    let argument = os::argument();
    let tree: Tuple = argument.try_into().unwrap();
    assert_eq!(tree.len(), 2);
    let x: Word = tree.get(0).try_into().unwrap();
    let y: Word = tree.get(1).try_into().unwrap();

    let z = x.read() + y.read();

    os::exit(Word::new(z));
}
