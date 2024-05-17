.intel_syntax noprefix
.code64

.section ".text"

.globl long_mode
long_mode:
  // this code executes on all CPUs

  mov eax, 0x01
  cpuid
  shr ebx, 24

  # clear argument registers
  xor rdi, rdi
  xor rsi, rsi
  xor rdx, rdx
  xor rcx, rcx
  xor r8, r8
  xor r9, r9

  # arg0 (dil): processor ID    (u8)
  # arg1 (sil): boot processor? (bool)
  # arg2 (dl): number of cores (u8)
  # arg3 (rcx): multiboot info  (void *)
  # arg4  (r8): ???
  # arg5  (r9): ???
  mov dil, bl
  cmp dil, bsp_id[rip]
  setz sil
  mov dl, ncores[rip]
  mov ecx, multiboot_information[rip]
  mov r8, 0xFFFF800000000000
  or rcx, r8

  # clear unused registers
  xor rbx, rbx
  xor rbp, rbp
  xor rsp, rsp
  xor r8, r8
  xor r9, r9
  xor r10, r10
  xor r11, r11
  xor r12, r12
  xor r13, r13
  xor r14, r14
  xor r15, r15

  # jump to 2MB mark
  mov rax, 0xFFFF800000200000
  jmp rax

.section ".data"
.extern multiboot_information
.globl bsp_id
.globl ncores

mutex: .byte 0
bsp_id: .byte 0
ncores: .byte 0
