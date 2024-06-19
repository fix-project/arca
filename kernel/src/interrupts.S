.intel_syntax noprefix
.altmacro
.section ".text"

.extern isr_entry
.extern syscall_entry

.globl syscall_handler
syscall_handler:
  // rflags -> r11
  // rip -> rcx
  // we are coming from user mode, so switch to kernel GS
  swapgs
  mov gs:[0x10], rsp
  mov rsp, gs:[0x08]
  push gs:[0x10]  # rsp
  push r11        # rflags
  push rcx        # rip

  push r15
  push r14
  push r13
  push r12
  push r10
  push  r9
  push  r8
  push rbp
  push rdi
  push rsi
  push rdx
  push rbx
  push rax
  lea  rdi, [rsp]
  call syscall_entry
  jmp 1f

.globl syscall_exit
syscall_exit:
  lea  rsp, [rdi]
1:
  pop  rax
  pop  rbx
  pop  rdx
  pop  rsi
  pop  rdi
  pop  rbp
  pop  r8 
  pop  r9 
  pop  r10
  pop  r12
  pop  r13
  pop  r14
  pop  r15

  pop  rcx
  pop  r11
  pop  rsp

  // we are returning to user code, so switch GS back
  swapgs
  sysretq

isr_stub:
  // ss                   0
  // rsp                  8
  // rflags              10
  // cs                  18
  // rip                 20
  // error code          28
  // isr number          30
  testq 0x18[rsp], 0b11
  jz 1f
  // if we are coming from user mode, switch to kernel GS
  swapgs
1:
  // ss                   0
  // rsp                  8
  // rflags              10
  // cs                  18
  // rip                 20
  xchg r15, 0x8[rsp]  // 28 (replaces error code)
  xchg r14, 0x0[rsp]  // 30 (replaces isr number)
  push r13            // 38
  push r12            // 40
  push r11            // 48
  push r10            // 50
  push  r9            // 58
  push  r8            // 60
  push rbp            // 68
  push rdi            // 70
  push rsi            // 78
  push rdx            // 80
  push rcx            // 88
  push rbx            // 90
  push rax            // 98

  lea  rdx, [rsp]
  mov  rdi, r14
  mov  rsi, r15
  call isr_entry
  jmp 1f

.globl isr_exit
isr_exit:
  lea  rsp, [rdi]
1:
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

  // if we are returning to user mode, switch back to the user GS
  testq 0x8[rsp], 0b11
  jz 1f
  swapgs
1:
  iretq

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
