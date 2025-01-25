#![no_std]
#![no_main]

use core::arch::global_asm;

extern crate user;

global_asm!(
    "
    .globl _start
    _start:
        int3
    "
);
