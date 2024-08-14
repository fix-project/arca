.intel_syntax noprefix
.code64

# rbx: multiboot info (void *)

.globl _start
.globl _bsp_start
.globl _ap_start
.extern NEXT_STACK_ADDR
.extern NEXT_CPU_READY
.extern _rsstart_bsp
.extern _rsstart_ap
.extern _rscontinue
.section ".text.start"
_start:
_bsp_start:
  # set up a temporary stack
  mov rsp, 0x0007FFF8
  cmp rbx, 0
  je halt
  mov rdi, rbx
  call _rsstart_bsp
  # _rsstart will allocate a "real" stack and return the top address
  mov rsp, rax
  sub rsp, 0x8
  call _rscontinue
  jmp halt

_ap_start:
  mov rsp, qword ptr NEXT_STACK_ADDR[rip]
  mov byte ptr NEXT_CPU_READY[rip], 1
  call _rsstart_ap
  call _rscontinue
  jmp halt

halt:
  cli
  hlt
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

.globl set_pt
set_pt:
  mov cr3, rdi
  ret

.globl get_pt
get_pt:
  mov rax, cr3
  ret
