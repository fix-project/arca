#![no_std]
#![no_main]

extern crate user;

unsafe extern "C" {
    static _end: core::ffi::c_void;
}

fn align(unaligned_val: usize, alignment: usize) -> usize {
    let mask = alignment - 1;
    unaligned_val + (-(unaligned_val as isize) as usize & mask)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(1);
    let ptr = &raw const _end;
    let ptr = (align(ptr as usize, 4096) + 8192) as *const u64;

    let mut count: u64 = 0;
    user::syscall::prompt(0); // Pages to map
    user::syscall::read_word(0, &mut count);
    if count > 0 {
        unsafe {
            user::syscall::map_new_pages(ptr as *const _, count as usize);
        }
    }
    let mut unified: u64 = 0;
    user::syscall::prompt(0); // unified?
    user::syscall::read_word(0, &mut unified);
    if unified != 0 {
        loop {
            user::syscall::return_continuation();
        }
    } else {
        loop {
            if user::syscall::continuation(0) == 0 {
                user::syscall::exit(0);
            }
        }
    }
}
