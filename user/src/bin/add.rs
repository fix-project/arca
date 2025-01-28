#![no_std]
#![no_main]

extern crate user;

/// Add 1 to a 64-bit integer.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(2);

    let mut x = [0; 8];
    let mut y = [0; 8];

    user::syscall::argument(0);
    user::syscall::read_tree(0, &[0, 1]);
    // let result = user::syscall::read_blob(0, unsafe {core::slice::from_raw_parts_mut(0xff as *mut _, 8)});
    user::syscall::read_blob(0, &mut x);
    user::syscall::read_blob(1, &mut y);
    let x = u64::from_ne_bytes(x);
    let y = u64::from_ne_bytes(y);
    let z = x + y;
    let bytes = z.to_ne_bytes();
    user::syscall::create_blob(0, &bytes);
    user::syscall::exit(0);
}
