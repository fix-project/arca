#![no_std]
#![no_main]

extern crate user;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(1);

    let mut x = [0; 8];
    user::syscall::argument(0);
    user::syscall::read_blob(0, &mut x);
    let x = u64::from_ne_bytes(x);

    let result = user::syscall::continuation(0);
    if result != defs::error::CONTINUED {
        user::syscall::exit(0);
    }

    let mut y = [0; 8];
    user::syscall::argument(0);
    user::syscall::read_blob(0, &mut y);
    let y = u64::from_ne_bytes(y);

    let z = x + y;
    let bytes = z.to_ne_bytes();
    user::syscall::create_blob(0, &bytes);
    user::syscall::exit(0);
}
