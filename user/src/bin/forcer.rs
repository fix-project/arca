#![no_std]
#![no_main]

extern crate user;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(2);
    user::syscall::prompt(1); // Thunk
    let mut count: u64 = 0;
    user::syscall::prompt(0); // Count
    user::syscall::read_word(0, &mut count);
    for _ in 0..count {
        user::syscall::force(1);
    }
    user::syscall::exit(0);
}
