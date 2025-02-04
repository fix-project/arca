#![no_std]
#![no_main]

extern crate user;

/// Try to corrupt kernel data, which should fail.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(1);
    user::syscall::prompt(0);
    let result = user::syscall::read_blob(0, unsafe {
        core::slice::from_raw_parts_mut(0xff as *mut _, 8)
    });
    if result >= 0 {
        unsafe {
            core::arch::asm!("int3");
        }
    }
    user::syscall::null(0);
    user::syscall::exit(0);
}
