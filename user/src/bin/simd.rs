#![no_std]
#![no_main]
use core::arch::global_asm;

#[allow(unused_imports)]
use user::prelude::*;

global_asm! {
    r#"
.globl _rsstart
_rsstart:
    // get arg (1)
    mov rax, 5
    syscall
    mov rdi, rax

    // read blob (1)
    mov rax, 13
    mov rsi, 0
    lea rdx, [x]
    mov r10, 32
    syscall

    // get arg (2)
    mov rax, 5
    syscall
    mov rdi, rax

    // read blob (2)
    mov rax, 13
    mov rsi, 0
    lea rdx, [y]
    mov r10, 32
    syscall

    // add blobs
    vmovdqu ymm0, ymmword ptr [x]
        // yield
        mov rax, 0
        syscall
    vmovdqu ymm1, ymmword ptr [y]
        // yield
        mov rax, 0
        syscall
    vpaddb ymm2, ymm0, ymm1
        // yield
        mov rax, 0
        syscall
    vmovdqu ymmword ptr [z], ymm2
        // yield
        mov rax, 0
        syscall

    // create blob
    mov rax, 8
    lea rdi, [z]
    mov rsi, 32
    syscall

    // exit
    mov rdi, rax
    mov rax, 3
    syscall

x:
    .zero 32

y:
    .zero 32

z:
    .zero 32
    "#
}
