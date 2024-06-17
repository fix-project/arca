.intel_syntax noprefix
.altmacro
.section ".text"

.extern isr_handler

.macro def_isr_stub_no_error num
isr_stub_\num:
  push rax
  push rbx
  push rcx
  push rdx
  push rsi
  push rdi
  push rbp
  push r8
  push r9
  push r10
  push r11
  push r12
  push r13
  push r14
  push r15
  pushfq
  mov rdi, \num
  call isr_handler
  popfq
  pop r15
  pop r14
  pop r13
  pop r12
  pop r11
  pop r10
  pop r9
  pop r8
  pop rbp
  pop rdi
  pop rsi
  pop rdx
  pop rcx
  pop rbx
  pop rax
  iretq
.endm

.set i,0
.rept 256
    def_isr_stub_no_error %i
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
