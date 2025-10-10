.intel_syntax noprefix

.section .text.start
.extern _rsstart
.extern __stack_top
.globl _start
_start:
  lea rsp, [__stack_top]
  call _rsstart
.halt:
  int3
  jmp .halt
