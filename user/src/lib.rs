#![no_std]
#![allow(unused)]

extern crate defs;

pub mod syscall {
    use core::arch::{asm, global_asm};

    global_asm!(
        "
    .globl syscall
    syscall:
        mov r10, rcx
        syscall
        ret
    "
    );

    use defs::syscall::*;

    unsafe extern "C" {
        fn syscall(num: u64, ...) -> u64;
    }

    pub fn noop() -> ! {
        unsafe { syscall(NOOP) };
        unreachable!();
    }

    pub fn exit(result: isize) -> ! {
        unsafe { syscall(EXIT, result) };
        unreachable!();
    }

    pub fn force(i: isize) -> isize {
        unsafe { syscall(EXIT, i) as isize }
    }

    pub fn argument() -> isize {
        unsafe { syscall(ARGUMENT) as isize }
    }

    pub fn eq(a: isize, b: isize) -> bool {
        unsafe { syscall(EQ, a, b) == 1 }
    }

    pub fn find(haystack: isize, needle: isize) -> isize {
        unsafe { syscall(FIND, haystack, needle) as isize }
    }

    pub fn len(value: isize) -> usize {
        unsafe { syscall(LEN, value) as usize }
    }

    pub fn atom_create(data: &[u8]) -> isize {
        unsafe { syscall(ATOM_CREATE, data.as_ptr(), data.len()) as isize }
    }

    pub fn blob_create(data: &[u8]) -> isize {
        unsafe { syscall(BLOB_CREATE, data.as_ptr(), data.len()) as isize }
    }

    pub fn blob_read(blob: isize, buffer: &mut [u8], offset: usize) -> usize {
        unsafe { syscall(BLOB_READ, blob, buffer.as_ptr(), buffer.len(), offset) as usize }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
