#![no_std]
#![no_main]

extern crate user;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(2);

    let arg = 0;
    let output = 1;
    let mut bytes = [0; 8];

    user::syscall::argument(arg);
    user::syscall::read_blob(arg, &mut bytes);
    let x = u64::from_ne_bytes(bytes);
    let y = x + 1;
    let bytes = y.to_ne_bytes();
    user::syscall::create_blob(output, &bytes);
    user::syscall::exit(output);
}
