#![no_std]
#![no_main]

use core::arch::asm;

extern crate user;

use user::prelude::*;

/// Takes a function which requires an n-tuple and the number n, and returns an n-ary function
/// which evaluates to the same thing.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let DynValue::Lambda(f) = os::prompt().into() else {
        panic!("first argument should be a function");
    };
    let DynValue::Word(n) = os::prompt().into() else {
        panic!("second argument should be a word");
    };
    let n = n.read() as usize;

    if n > 4 {
        unsafe { asm!("int3") }
    }

    let mut args: [Ref<Value>; 4] = Default::default();

    // read arguments
    for x in args.iter_mut().take(n) {
        *x = os::prompt();
    }

    let tree = os::tree(&mut args[..n]);
    let y = f.apply(tree.into());
    os::tailcall(y);
}
