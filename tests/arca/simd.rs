#![no_std]
#![no_main]

extern crate user;

use core::arch::global_asm;

// Keep SIMD values live across syscalls and a suspended argument request without
// compiler-generated spills or vzeroupper masking failures in context switching.
global_asm!(
    r#"
.globl _rsstart
_rsstart:
    .irp index,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
    vmovdqu [rip + output + 32 * \index], ymm\index
    .endr
    stmxcsr [rip + output + 512]
    fnstcw [rip + output + 516]

1:
    mov eax, {argument}
    syscall
    test rax, rax
    js 1b
    mov rdi, rax
    mov eax, {read}
    xor esi, esi
    lea rdx, [rip + input]
    mov r10d, 32
    syscall

    vmovdqu ymm0, [rip + input]
    .irp index,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
    vpaddb ymm\index, ymm0, [rip + increments + 32 * \index]
    .endr
    ldmxcsr [rip + simd_mxcsr]
    fld1

    mov eax, {nop}
    syscall
2:
    mov eax, {argument}
    syscall
    test rax, rax
    js 2b
    mov eax, {nop}
    syscall

    .irp index,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
    vmovdqu [rip + output + 518 + 32 * \index], ymm\index
    .endr
    stmxcsr [rip + output + 1030]
    fstp qword ptr [rip + output + 1034]
    mov eax, {create_blob}
    lea rdi, [rip + output]
    mov esi, 1042
    syscall
    mov rdi, rax
    mov eax, {exit}
    syscall
    ud2

.section .rodata
simd_mxcsr:
    .long 0x3f80
increments:
    .irp index,0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15
    .fill 32,1,\index
    .endr
.section .bss
input:
    .zero 32
output:
    .zero 1042
"#,
    argument = const arcane::__NR_get_argument,
    read = const arcane::__NR_read,
    nop = const arcane::__NR_nop,
    create_blob = const arcane::__NR_create_blob,
    exit = const arcane::__NR_exit,
);
