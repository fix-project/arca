.intel_syntax noprefix
.code64

.section ".text"

.globl long_mode
long_mode:
  // this code executes on all CPUs

  mov eax, 0x01
  cpuid
  shr ebx, 24

  # arg0 (rdi): processor ID    (u8)
  # arg1 (rsi): boot processor? (bool)
  # arg2 (rdx): number of cores (u8)
  # arg3 (rcx): multiboot info  (void *)
  # arg4  (r8): ???
  # arg5  (r9): ???
  mov dil, bl
  cmp dil, bsp_id[rip]
  setz sil
  mov dl, ncores[rip]
  mov ecx, multiboot_information[rip]

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
