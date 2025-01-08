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
