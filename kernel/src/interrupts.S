.intel_syntax noprefix
.altmacro
.section ".text"

.extern isr_handler

.macro def_isr_stub num
isr_stub_\num:
  swapgs
  mov dx, 0xe9
  lea rsi, message[rip]
  mov ecx, 7
  rep outsb
0:
  pause
  jmp 0b
.endm

.set i,0
.rept 256
    def_isr_stub %i
    .set i,i+1
.endr

.section ".data"

.macro addr_isr_stub num
.quad isr_stub_\num
.endm

.globl isr_table
isr_table:

.set i,0
.rept 256
    addr_isr_stub %i
.set i,i+1
.endr

message: .ascii "Error!\n"
