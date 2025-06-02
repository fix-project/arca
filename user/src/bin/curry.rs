#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Takes a function which requires an n-tuple and the number n, and returns an n-ary function
/// which evaluates to the same thing.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let f: Ref<Lambda> = os::prompt()
        .try_into()
        .expect("first argument should be a function");
    let n: Ref<Word> = os::prompt()
        .try_into()
        .expect("second argument should be a word");
    let n = n.read() as usize;

    let mut tree = os::tree(n);

    // read arguments
    for i in 0..n {
        tree.put(i, os::prompt());
    }

    let y = f.apply(tree.into());
    os::tailcall(y);
}
