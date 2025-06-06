.intel_syntax noprefix

.extern fmain
.extern __stack_top
.globl _start
_start:
  lea rsp, __stack_top[rip]
  call fmain
  int3

.globl putc
putc:
  mov rax, rdi
  mov dx, 0xe9
  out dx, al
  ret

.globl syscall
syscall:
    mov r10, rcx
    syscall
    ret
