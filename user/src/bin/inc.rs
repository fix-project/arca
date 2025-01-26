#![no_std]
#![no_main]

extern crate user;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let arg = user::syscall::argument();
    let mut bytes = [0; 8];
    user::syscall::blob_read(arg, &mut bytes, 0);
    let x = u64::from_ne_bytes(bytes);
    let y = x + 1;
    let bytes = y.to_ne_bytes();
    let blob = user::syscall::blob_create(&bytes);
    user::syscall::exit(blob);
}
