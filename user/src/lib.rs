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

    use defs::{error, syscall, syscall::*, types};

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

    /// # Safety
    /// The operand must be of type word.
    pub unsafe fn read_word_unchecked(src: u64, result: &mut u64) -> i64 {
        unsafe { syscall(READ, src, result) }
    }

    /// # Safety
    /// The operand must be of type blob.
    pub unsafe fn read_blob_unchecked(src: u64, buffer: &mut [u8]) -> i64 {
        unsafe { syscall(READ, src, buffer.as_ptr(), buffer.len()) }
    }

    /// # Safety
    /// The operand must be of type tree.
    pub unsafe fn read_tree_unchecked(src: u64, keys: &[u64]) -> i64 {
        unsafe { syscall(READ, src, keys.as_ptr(), keys.len()) }
    }

    pub fn get_type(arg: u64) -> i64 {
        unsafe { syscall(TYPE, arg) }
    }

    pub fn read_word(src: u64, result: &mut u64) -> i64 {
        unsafe {
            assert_eq!(get_type(src) as u32, types::WORD);
            read_word_unchecked(src, result)
        }
    }

    pub fn read_blob(src: u64, buffer: &mut [u8]) -> i64 {
        unsafe {
            assert_eq!(get_type(src) as u32, types::BLOB);
            read_blob_unchecked(src, buffer)
        }
    }

    pub fn read_tree(src: u64, keys: &[u64]) -> i64 {
        unsafe {
            assert_eq!(get_type(src) as u32, types::TREE);
            read_tree_unchecked(src, keys)
        }
    }

    pub fn create_word(dst: u64, word: u64) -> i64 {
        unsafe { syscall(CREATE_WORD, dst, word) }
    }

    pub fn create_blob(dst: u64, buffer: &[u8]) -> i64 {
        unsafe { syscall(CREATE_BLOB, dst, buffer.as_ptr(), buffer.len()) }
    }

    pub fn create_tree(dst: u64, buffer: &[u64]) -> i64 {
        unsafe { syscall(CREATE_TREE, dst, buffer.as_ptr(), buffer.len()) }
    }

    pub unsafe fn map_new_pages(ptr: *const (), count: usize) -> i64 {
        unsafe { syscall(MAP_NEW_PAGES, ptr, count) }
    }

    pub fn continuation(dst: u64) -> i64 {
        unsafe { syscall(CONTINUATION, dst) }
    }

    pub fn return_continuation() -> i64 {
        unsafe { syscall(RETURN_CONTINUATION) }
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

    pub fn force(thunk: u64) -> i64 {
        unsafe { syscall(FORCE, thunk) }
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
        unsafe {
            core::arch::asm!("ud2");
        }
        core::hint::spin_loop();
    }
}
