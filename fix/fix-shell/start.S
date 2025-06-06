.intel_syntax noprefix

.extern fmain
.extern __stack_top
.globl _start
_start:
  lea rsp, __stack_top[rip]
  call fmain
  int3
