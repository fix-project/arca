#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let argument = os::prompt();
    let mut tree: Ref<Tree> = argument.try_into().unwrap();
    assert_eq!(tree.len(), 2);
    let x: Ref<Word> = tree.take(0).try_into().unwrap();
    let y: Ref<Word> = tree.take(1).try_into().unwrap();

    let z = x.read() + y.read();

    os::exit(os::word(z));
}
