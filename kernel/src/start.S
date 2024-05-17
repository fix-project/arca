.intel_syntax noprefix
.code64

# arg0 (dil): processor ID    (u8)
# arg1 (sil): boot processor? (bool)
# arg2 (dl): number of cores (u8)
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
