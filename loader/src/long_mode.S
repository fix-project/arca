.intel_syntax noprefix
.code64

.section ".text"

.globl long_mode
long_mode:
  # this code executes on all CPUs

  # physical to virtual mask
  mov r8, 0xFFFF800000000000

  # get the CPU's local APIC ID and BSP status
  mov ecx, 0x1B # IA32_APIC_BASE
  rdmsr
  # switch the APIC to x2APIC mode
  or eax, 0b11 << 10
  wrmsr

  # store BSP status in rsi
  mov rsi, rax
  shr rsi, 8
  and rsi, 1

  # get local APIC address
  mov eax, 0x0B
  cpuid
  mov edi, edx

  # arg0 (rdi): processor ID     (u32)
  # arg1 (rsi): BSP?             (bool)
  # arg2 (rdx): number of cores  (u32)
  # arg3 (rcx): multiboot info   (void *)
  # arg4  (r8): ???
  # arg5  (r9): ???
  mov edx, ncores[rip]
  mov ecx, multiboot_information[rip]
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
.globl ncores

ncores: .long 0
