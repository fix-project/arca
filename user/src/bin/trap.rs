#![no_std]
#![no_main]

use core::arch::global_asm;

extern crate user;

unsafe extern "C" {
    /// Immediately force a trap using the breakpoint instruction (interrupt #3).  This program
    /// does not need a stack or any data or any system calls, just a mapped text segment.
    fn _start() -> !;
}
global_asm!(
    "
    .globl _start
    _start:
        int3
    "
);
