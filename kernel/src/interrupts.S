.intel_syntax noprefix
.altmacro
.section ".text"

.extern isr_handler

.globl syscall_handler_asm
syscall_handler_asm:
  // rflags -> r11
  // rip -> rcx
  // we are coming from user mode, so switch to kernel GS
  swapgs
  mov gs:[SAVED_RSP@tpoff], rsp
  mov rsp, gs:[INTERRUPT_STACK@tpoff]
  push 0x23                  # ss
  push gs:[SAVED_RSP@tpoff]  # rsp
  push r11                   # rflags
  push 0x2B                  # cs
  push rcx                   # rip
  push 0                     # error code
  push 0x100                 # isr number
  # fall through to ISR body

isr_body:
  # ss         0
  # rsp        8
  # rflags    10
  # cs        18
  # rip       20
  # err code  28 -> gs base
  # isr num   30 -> fs base
  push r15  # 38
  push r14  # 40
  push r13  # 48
  push r12  # 50
  push r11  # 58
  push r10  # 60
  push  r9  # 68
  push  r8  # 70
  push rbp  # 78
  push rdi  # 80
  push rsi  # 88
  push rdx  # 90
  push rcx  # 98
  push rbx  # a0
  push rax  # a8

  // just use the user segments, saves 20ns
  // mov ax, ss
  // mov ds, ax
  // mov es, ax

  // TODO: this costs 20ns
  rdgsbase rsi
  xchg rsi, (0xA8 - 0x28)[rsp]
  rdfsbase rdi
  xchg rdi, (0xA8 - 0x30)[rsp]

  // set FS to GS for the kernel (costs 5ns)
  mov rax, gs:[0x00]
  wrfsbase rax

  lea  rdx, [rsp]
  call isr_handler

  // if we are returning to user mode, switch back to the user GS
  mov rax, (0xA8-0x18)[rsp]
  test rax, 0b11
  jz 1f
  swapgs
1:
  
  mov rax, (0xA8 - 0x30)[rsp]
  wrfsbase rax
  mov rax, (0xA8 - 0x28)[rsp]
  wrgsbase rax

  mov ax, (0xA8 - 0x00)[rsp]
  mov ds, ax
  mov es, ax

  pop  rax
  pop  rbx
  pop  rcx
  pop  rdx
  pop  rsi
  pop  rdi
  pop  rbp
  pop  r8 
  pop  r9 
  pop  r10
  pop  r11
  pop  r12
  pop  r13
  pop  r14
  pop  r15
  add  rsp, 16
  iretq

isr_stub:
  push rax
  mov rax, 0x08[rsp]
  test rax, 0b11
  jz 1f
  // if we are coming from user mode, switch to kernel GS
  swapgs
1:
  pop rax
  jmp isr_body

.macro def_stub_error num
isr_stub_\num:
  push \num
  jmp isr_stub
.endm

.macro def_stub_no_error num
isr_stub_\num:
  push 0
  push \num
  jmp isr_stub
.endm

def_stub_no_error 0
def_stub_no_error 1
def_stub_no_error 2
def_stub_no_error 3
def_stub_no_error 4
def_stub_no_error 5
def_stub_no_error 6
def_stub_no_error 7
def_stub_error    8
def_stub_no_error 9
def_stub_error    10
def_stub_error    11
def_stub_error    12
def_stub_error    13
def_stub_error    14
def_stub_no_error 15
def_stub_no_error 16
def_stub_error    17
def_stub_no_error 18
def_stub_no_error 19
def_stub_no_error 20
def_stub_error    21
def_stub_no_error 22
def_stub_no_error 23
def_stub_no_error 24
def_stub_no_error 25
def_stub_no_error 26
def_stub_no_error 27
def_stub_no_error 28
def_stub_error    29
def_stub_error    30
def_stub_no_error 31

.set i,32
.rept 224
    def_stub_no_error %i
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
