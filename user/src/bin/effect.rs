#![no_std]
#![no_main]

extern crate user;

/// Performs a side effect and returns the result of the side effect unmodified.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(2);
    
    let effect = "effect";
    let x = effect.as_bytes();
    user::syscall::create_blob(0, x);

    user::syscall::prompt_effect(0, 1);
    user::syscall::exit(1);
}
