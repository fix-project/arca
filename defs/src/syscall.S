.intel_syntax noprefix

.globl syscall
syscall:
    mov r10, rcx
    syscall
    ret
