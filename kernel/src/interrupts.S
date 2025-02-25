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
  cli
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
  cli
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
  // sti
  ret

.globl isr_call_user
// isr_call_user(registers: RegisterFile) -> (u64, u64)
// return value: (event number, error code)
isr_call_user:
  cli
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
  test qword ptr 0x18[rsp], 0b11
  jz 1f
  swapgs
1:
  push r15
  push r14
  push r13
  push r12
  push r11
  push r10
  push r9
  push r8
  push rdi
  push rsi
  push rbp
  push 0x80[rsp]
  push rbx
  push rdx
  push rcx
  push rax

  lea rdi, [rsp]
  call isr_entry
  // we only get here if we're restoring back to the interrupted point

  pop rax
  pop rcx
  pop rdx
  pop rbx
  pop 0x80[rsp]
  pop rbp
  pop rsi
  pop rdi
  pop r8
  pop r9
  pop r10
  pop r11
  pop r12
  pop r13
  pop r14
  pop r15

  test qword ptr 0x18[rsp], 0b11
  jz 1f
  swapgs
1:
  add rsp, 0x10
  iretq

.globl isr_save_state_and_exit
isr_save_state_and_exit:
  mov r8, rdi
  mov r9, rsi

  # copy the continuation to the register save area since this stack is going to be destroyed
  mov rsi, rdx
  mov rdi, gs:[REGISTER_SAVE_AREA]
  sub rdi, 152
  mov rcx, 152
  rep movsb

  mov rsp, gs:[RESUME_RSP]
  mov rax, r8
  mov rdx, r9

  pop r15
  pop r14
  pop r13
  pop r12
  pop rbp
  pop rbx
  // sti
  ret

.macro def_stub_error num
isr_stub_\num:
  cli
  push \num
  jmp isr_stub
.endm

.macro def_stub_no_error num
isr_stub_\num:
  cli
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
