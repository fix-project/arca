.intel_syntax noprefix
.code64

# arg0 (edi): processor ID    (u32)
# arg1 (esi): BSP ID          (bool)
# arg2 (edx): number of cores (u32)
# arg3 (rcx): multiboot info  (void *)

.altmacro
.macro print c
  mov dx, 0xe9
  mov al, \c
  outb dx, al
.endm

.globl _start
.section ".text.start"
_start:
  # set up a 16KB stack for each processor (4MB total for 256 cores)
  lea rsp, _sstack[rip]
  mov rax, rdi
  inc rax
  shl rax, 14
  add rsp, rax
  sub rsp, 0x8
  call _rsstart
  # _rsstart will allocate a "real" stack and return the top address
  mov rsp, rax
  sub rsp, 0x8
  call _rscontinue
halt:
  jmp halt

.globl set_gdt
set_gdt:
  // load new GDT
  lgdt [rdi]

  // reload selectors
  mov ax, 0x23
  mov ds, ax
  mov es, ax
  mov ax, 0x10
  mov ss, ax

  // reload code selector
  mov rax, 0x08
  push rax
  lea rax, new_gdt[rip]
  push rax
  rex.w lret
new_gdt:
  ret
