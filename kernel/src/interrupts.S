.intel_syntax noprefix
.altmacro

/*
struct register_file {
  u64  registers[16];
  u64  rip;
  u64  flags;
  u64  mode;
};
*/

.section ".cdata"
RESUME_RSP: .quad 0
SAVED_RSP: .quad 0
REGISTER_SAVE_AREA: .quad 0

.section ".text"

.extern isr_entry

.globl syscall_call_user
// syscall_call_user(registers: RegisterFile) -> (u64, u64)
// return value: (event number, error code)
syscall_call_user:
  push rbx
  push rbp
  push r12
  push r13
  push r14
  push r15
  
  // done saving system state

  mov gs:[RESUME_RSP], rsp
  mov rsp, rdi
  pop rax
  pop rcx
  pop rdx
  pop rbx
  pop gs:[SAVED_RSP]
  pop rbp
  pop rsi
  pop rdi
  pop  r8
  pop  r9
  pop r10
  pop r11
  pop r12
  pop r13
  pop r14
  pop r15

  pop rcx
  pop r11
  // TODO: check that we should be going to user mode?
  add rsp, 8
  mov gs:[REGISTER_SAVE_AREA], rsp
  mov rsp, gs:[SAVED_RSP]
  swapgs
  sysretq

.globl syscall_handler
syscall_handler:
  // rflags -> r11
  // rip -> rcx
  // we are coming from user mode, so switch to kernel GS
  swapgs
  mov gs:[SAVED_RSP], rsp
  mov rsp, gs:[REGISTER_SAVE_AREA]

  push 1               # user mode
  push r11             # rflags
  push rcx             # rip

  push r15
  push r14
  push r13
  push r12
  push r11
  push r10
  push  r9
  push  r8
  push rdi
  push rsi
  push rbp
  push gs:[SAVED_RSP]
  push rbx
  push rdx
  push rcx
  push rax

  mov rsp, gs:[RESUME_RSP]
syscall_return_to_system:
  pop r15
  pop r14
  pop r13
  pop r12
  pop rbp
  pop rbx
  mov rax, 256  // exit due to syscall
  mov rdx, 0    // error code 0
  ret

.globl isr_call_user
// isr_call_user(registers: RegisterFile) -> (u64, u64)
// return value: (event number, error code)
isr_call_user:
  push rbx
  push rbp
  push r12
  push r13
  push r14
  push r15
  
  mov gs:[RESUME_RSP], rsp
  // done saving system state
  push 0x23         // ss
  push 0x20[rdi]    // rsp
  push 0x88[rdi]    // rflags
  push 0x2b         // cs
  push 0x80[rdi]    // rip
  mov gs:[SAVED_RSP], rsp

  mov rsp, rdi
  pop rax
  pop rcx
  pop rdx
  pop rbx
  add rsp, 8
  pop rbp
  pop rsi
  pop rdi
  pop  r8
  pop  r9
  pop r10
  pop r11
  pop r12
  pop r13
  pop r14
  pop r15

  add rsp, 24

  mov gs:[REGISTER_SAVE_AREA], rsp
  mov rsp, gs:[SAVED_RSP]
  swapgs
  iretq

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
  mov gs:[SAVED_RSP], rsp
  mov rsp, gs:[REGISTER_SAVE_AREA]
  push 1 // user mode
  jmp 2f
1:
  mov gs:[SAVED_RSP], rsp
  mov rsp, gs:[REGISTER_SAVE_AREA]
  push 0 // super mode
2:
  push 0 // flags (to be filled in later)
  push 0 // rip (to be filled in later)

  push r15
  push r14
  push r13
  push r12
  push r11
  push r10
  push  r9
  push  r8
  push rdi
  push rsi
  push rbp
  push 0 // rsp (to be filled in later)
  push rbx
  push rdx
  push rcx
  push rax

  mov rsi, rsp
  mov rsp, gs:[SAVED_RSP]

  pop r15 // isr number
  pop r14 // error code
  pop r12 // rip
  pop r13 // cs
  test r13, 0b11
  mov r13, 0
  jz 1f
  inc r13 // is user mode
1:
  pop r11 // flags
  pop r10 // rsp

  mov 0x20[rsi], r10
  mov 0x80[rsi], r12
  mov 0x88[rsi], r11

  mov rsp, gs:[RESUME_RSP]

  push r14
  push r15
  lea rdi, [rsp]
  call isr_entry
isr_return_to_system:
  pop rax // use exit code provided by system
  pop rdx // use error code provided by system

  pop r15
  pop r14
  pop r13
  pop r12
  pop rbp
  pop rbx
  ret

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
