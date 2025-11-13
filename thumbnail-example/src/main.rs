#![no_std]
#![no_main]

extern crate alloc;
extern crate user;

use user::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn _rsstart() -> ! {
    // // screw it, we hardcoding ... TODO(kmohr)
    // let image_indx: Word = Function::symbolic("get")
    //     // .apply(hostname)
    //     // .apply(port)
    //     .apply(file_path)
    //     .call_with_current_continuation()
    //     .try_into()
    //     .expect("add should return a word");

    let i: Word = Function::symbolic("get")
        .call_with_current_continuation()
        .try_into()
        .expect("should return a word");

    let mut n = 1;
    for j in 1..(i.read() + 1) {
        n *= j;
    }

    os::exit(Word::new(n));
}
