.intel_syntax noprefix

// Freestanding memory routines for the System V core/alloc libraries.
.section .text.memcpy, "ax"
.globl memcpy
memcpy:
  mov rax, rdi
  mov rcx, rdx
  rep movsb
  ret

.section .text.memmove, "ax"
.globl memmove
memmove:
  cmp rdi, rsi
  jbe memcpy
  mov rax, rdi
  test rdx, rdx
  jz 1f
  lea rdi, [rdi + rdx - 1]
  lea rsi, [rsi + rdx - 1]
  // Keep DF clear: an interrupt can enter the kernel in the middle of a copy.
2:
  mov cl, byte ptr [rsi]
  mov byte ptr [rdi], cl
  dec rsi
  dec rdi
  dec rdx
  jnz 2b
1:
  ret

.section .text.memset, "ax"
.globl memset
memset:
  mov r8, rdi
  mov eax, esi
  mov rcx, rdx
  rep stosb
  mov rax, r8
  ret

.section .text.memcmp, "ax"
.globl memcmp
.globl bcmp
memcmp:
bcmp:
  xor eax, eax
  test rdx, rdx
  jz 2f
1:
  movzx eax, byte ptr [rdi]
  movzx ecx, byte ptr [rsi]
  sub eax, ecx
  jne 2f
  inc rdi
  inc rsi
  dec rdx
  jnz 1b
2:
  ret

.section .note.GNU-stack, "", @progbits

.section .text.strlen, "ax"
.globl strlen
strlen:
  xor eax, eax
1:
  cmp byte ptr [rdi + rax], 0
  je 2f
  inc rax
  jmp 1b
2:
  ret
