.intel_syntax noprefix

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

.globl set_pt
set_pt:
  mov cr3, rdi
  ret

.globl get_pt
get_pt:
  mov rax, cr3
  ret

.globl copy_kernel_to_user_asm
.globl copy_user_to_kernel_asm
copy_kernel_to_user_asm:
copy_user_to_kernel_asm:
  // rdi: dst
  // rsi: src
  // rdx: len
  // return: 1 if successful, 0 if error

  cli

  lea rax, 1f[rip]
  mov gs:[SEGFAULT_ESCAPE_ADDR], rax

  mov rax, 0

  stac
  mov rcx, rdx
  rep movsb

  mov rax, 1

1:
  clac
  mov qword ptr gs:[SEGFAULT_ESCAPE_ADDR], 0

  // sti
  ret
