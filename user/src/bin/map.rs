#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Map a Lambda over a Tree.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let function: Ref<Lambda> = os::prompt()
        .try_into()
        .expect("incorrect argument type to map");
    let mut tree: Ref<Tree> = os::prompt()
        .try_into()
        .expect("incorrect argument type to map");

    let mut new_tree = os::tree(tree.len());

    for i in 0..tree.len() {
        let f = function.clone();
        new_tree.put(i, f.apply(tree.take(i)).into());
    }

    os::exit(new_tree);
}
