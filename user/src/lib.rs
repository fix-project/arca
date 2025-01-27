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

    pub fn resize(len: usize) {
        unsafe { syscall(RESIZE, len) };
    }

    pub fn exit(value: u64) -> ! {
        unsafe {
            syscall(EXIT);
            asm!("int3");
        }
        unreachable!();
    }

    pub fn argument(dst: u64) {
        unsafe { syscall(ARGUMENT, dst) };
    }

    pub fn read_blob(src: u64, buffer: &mut [u8]) -> bool {
        unsafe { syscall(READ, src, buffer.as_ptr(), buffer.len()) == 0 }
    }

    pub fn create_blob(dst: u64, buffer: &[u8]) -> bool {
        unsafe { syscall(CREATE_BLOB, dst, buffer.as_ptr(), buffer.len()) == 0 }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
