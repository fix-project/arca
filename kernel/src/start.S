.intel_syntax noprefix
.code64

# arg0 (rdi): processor ID    (u8)
# arg1 (rsi): boot processor? (bool)
# arg2 (rdx): number of cores (u8)
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

  # let the BSP run the init code
  cmp sil, 1
  je boot

  # APs: wait for the BSP to finish initializing the runtime
wait_for_init:
  mov al, rsstart_gate[rip]
  cmp al, 0
  je wait_for_init

boot:
  call rsstart
halt:
  jmp halt

.section ".data"
.global rsstart_gate
rsstart_gate: .byte 0
