.intel_syntax noprefix
.code64

# arg0 (edi): processor ID    (u32)
# arg1 (esi): BSP ID          (bool)
# arg2 (edx): number of cores (u32)
# arg3 (rcx): multiboot info  (void *)

.globl _start
.section ".text.start"
_start:
  # set up a 4KB stack for each processor (1MB total for 256 cores)
  mov rsp, 0xFFFF800000100000
  mov rax, rdi
  inc rax
  shl rax, 12
  add rsp, rax
  sub rsp, 0x8
  call rsstart
halt:
  jmp halt
