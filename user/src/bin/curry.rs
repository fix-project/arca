#![no_std]
#![no_main]

use core::arch::asm;

extern crate user;

/// Takes a function which requires an n-tuple and the number n, and returns an n-ary function
/// which evaluates to the same thing.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(5);
    user::syscall::prompt(0);
    user::syscall::prompt(1);
    let mut count: u64 = 0;
    user::syscall::read_word(1, &mut count);
    let count = count as usize;

    if count > 4 {
        unsafe { asm!("int3") }
    }

    // read arguments
    for i in 0..count {
        user::syscall::prompt((i + 1) as u64);
    }
    let args = &[1, 2, 3, 4];

    user::syscall::create_tree(1, &args[0..count]);
    user::syscall::apply(0, 1);
    user::syscall::tailcall(0);
    loop {
        core::hint::spin_loop();
    }
}
