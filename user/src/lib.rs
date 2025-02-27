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
        fn syscall(num: u64, ...) -> i64;
    }

    pub fn resize(len: usize) -> i64 {
        unsafe { syscall(RESIZE, len) }
    }

    pub fn null(dst: u64) -> i64 {
        unsafe { syscall(NULL, dst) }
    }

    pub fn exit(value: u64) -> ! {
        unsafe {
            syscall(EXIT, value);
            asm!("ud2");
        }
        unreachable!();
    }

    pub fn read_blob(src: u64, buffer: &mut [u8]) -> i64 {
        unsafe { syscall(READ, src, buffer.as_ptr(), buffer.len()) }
    }

    pub fn read_tree(src: u64, keys: &[u64]) -> i64 {
        unsafe { syscall(READ, src, keys.as_ptr(), keys.len()) }
    }

    pub fn create_blob(dst: u64, buffer: &[u8]) -> i64 {
        unsafe { syscall(CREATE_BLOB, dst, buffer.as_ptr(), buffer.len()) }
    }

    pub fn create_tree(dst: u64, buffer: &[u64]) -> i64 {
        unsafe { syscall(CREATE_TREE, dst, buffer.as_ptr(), buffer.len()) }
    }

    pub fn continuation(dst: u64) -> i64 {
        unsafe { syscall(CONTINUATION, dst) }
    }

    pub fn prompt(dst: u64) -> i64 {
        unsafe { syscall(PROMPT, dst) }
    }

    pub fn perform(src: u64, dst: u64) -> i64 {
        unsafe { syscall(PERFORM, src, dst) }
    }

    pub fn apply(lambda: u64, arg: u64) -> i64 {
        unsafe { syscall(APPLY, lambda, arg) }
    }

    pub fn show(msg: &str, idx: u64) -> i64 {
        unsafe { syscall(SHOW, msg.as_ptr(), msg.len(), idx) }
    }

    pub fn log(msg: &str) -> i64 {
        unsafe { syscall(LOG, msg.as_ptr(), msg.len()) }
    }

    pub fn tailcall(thunk: u64) -> i64 {
        unsafe { syscall(TAILCALL, thunk) }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
