// based on https://wiki.osdev.org/Setting_Up_Long_Mode

.intel_syntax noprefix
.code32

.equ MAGIC, 0x1BADB002
.equ FLAGS, 0x00000000

.section ".multiboot"
.align 32
.long MAGIC
.long FLAGS
.long -(MAGIC)

.extern _estack
.extern _cstart

.section ".text.start"
.globl _start
_start:
  // this is the entry point for the Bootstrap Processor (the main CPU)
  // we set up a minimal C runtime so we can set up paging and boot the other CPUs from C

  // EBX contains a multiboot info structure
  mov [multiboot_information], ebx
  lea esp, _estack
  call _cstart
0:
  hlt
  jmp 0b

.section ".data"
.globl multiboot_information
multiboot_information: .long 0
