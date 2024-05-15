// based on https://wiki.osdev.org/Setting_Up_Long_Mode

.intel_syntax noprefix
.code32

.equ MAGIC, 0x1BADB002
.equ FLAGS, 0x00000000

.section ".multiboot"
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
  lea esi, [error_unreachable]
  call puts
  call halt

puts: ; esi = string
  cmp byte ptr [esi], 0
  jz .exit_puts
  mov dx, 0x3f8
  mov al, [esi]
  out dx, al
  inc esi
  jmp puts
.exit_puts:
  ret

halt:
  lea esi, [error_halting]
  call puts
.loop:
  hlt
  jmp .loop

.section ".data"
.globl multiboot_information
multiboot_information: .long 0

.section ".rodata"
error_unreachable: .asciz "ERROR (loader): unreachable\n"
error_halting: .asciz "ERROR (loader): loader is halting\n"
