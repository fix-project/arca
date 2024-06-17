.intel_syntax noprefix
.code16

.section ".text.trampoline"

.align 4096
.globl trampoline
trampoline:
  // this is the entry point for all the Application Processors (every CPU except the boot one)
  jmp main

.align 4
gdtr_32:
.word (gdt_end - gdt - 1)
.long gdt

gdt:
gdt_null:
  .quad 0
gdt_code:
  .word 0xffff
  .word 0x0000
  .byte 0x0000
  .byte 0b10011011
  .byte 0b11001111
  .byte 0x00
gdt_data:
  .word 0xffff
  .word 0x0000
  .byte 0x0000
  .byte 0b10010011
  .byte 0b11001111
  .byte 0x00
gdt_end:

main:
  // disable interrupts
  cli

  // enable A20 line
  in al, 0x92
  or al, 2
  out 0x92, al

  // enable protected mode
  mov eax, cr0
  or al, 1
  mov cr0, eax

  // load 32-bit GDT
  lgdt [gdtr_32]

  // reload selectors
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax
  mov ss, ax

.att_syntax
  // jump to 32-bit code
  ljmp $0x8, $protected_mode
.intel_syntax noprefix

.code32
.globl protected_mode
protected_mode:
  // both the BSP and the APs run this code to switch to long mode

  // enable SSE
  mov eax, cr0
  and ax, (~0b100) // disable x87 emulation
  or ax, 0x1 // enable monitor coprocessor
  mov cr0, eax

  // enable physical address extension and paging
  // also enable SSE OS support at the same time
  // also enable FSGSBASE
  // also enable XSAVE
  mov eax, 0x506a0
  mov cr4, eax

  // set up page table
  lea eax, [pml4]
  mov cr3, eax

  // enable long mode and system calls in EFER
  mov ecx, 0xC0000080
  rdmsr
  or eax, 0x00000101
  wrmsr

  // enable paging
  mov eax, cr0
  or eax, 0x80000001
  mov cr0, eax

  out dx, al

  // load 64-bit GDT
  lgdt [gdtr]

  // reload selectors
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax
  mov ss, ax

.att_syntax
  // jump to 32-bit code
  ljmp $0x8, $long_mode_entry
.intel_syntax noprefix

.code64
long_mode_entry:
  lea rax, long_mode[rip]
  mov rbx, 0xFFFF800000000000
  or rax, rbx
  jmp rax

.extern long_mode
