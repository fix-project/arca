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

/// Grow this Lambda's memory.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    user::syscall::resize(1);
    let ptr = &raw const _end;
    let ptr = (align(ptr as usize, 4096) + 8192) as *const u64;

    unsafe {
        user::syscall::map_new_pages(ptr as _, 24);
        user::syscall::create_word(0, ptr.read());
    }

    user::syscall::exit(0);
}
