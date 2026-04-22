.intel_syntax noprefix 

/*
 * Cooperatively switches from one kthread to another.
 *
 * params:
 *   rdi: new stack pointer
 *   rsi: pointer to old stack pointer
 */
.globl cswitch
cswitch:
  cli
  push rbx
  push rbp
  push r12
  push r13
  push r14
  push r15

  mov [rsi], rsp
  mov rsp, rdi

  pop r15
  pop r14
  pop r13
  pop r12
  pop rbp
  pop rbx

  sti
  ret

.globl kthread_init
kthread_init:
  pop rax
  pop rdi
  jmp rax
