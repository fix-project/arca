.intel_syntax noprefix
.code64

.section ".text"

.globl long_mode
long_mode:
  // this code executes on all CPUs

0:
  // lock the mutex
  mov al, 0
  mov bl, 1
  lock cmpxchg byte ptr [mutex], bl
  cmp al, 1
  jne 1f
  pause
  jmp 0b

1:
  mov eax, 0x01
  cpuid
  shr ebx, 24
  mov r15b, bl

  cmp r15b, [bsp_id]
  je 2f
  lea rsi, [message_ap]
  jmp 3f
2:
  lea rsi, [message_bsp]
3:
  mov dx, 0x3f8
  mov ecx, 0x10
  rep outsb

  mov bl, r15b
  shr bl, 4
  mov cl, r15b
  and cl, 0xf

  mov dx, 0x3f8
  mov al, [base16 + ebx]
  out dx, al
  mov al, [base16 + ecx]
  out dx, al
  mov al, '/'
  out dx, al

  mov bl, [ncores]
  shr bl, 4
  mov cl, [ncores]
  and cl, 0xf
  mov al, [base16 + ebx]
  out dx, al
  mov al, [base16 + ecx]
  out dx, al

  lea rsi, [message_end]
  mov dx, 0x3f8
  mov ecx, 0x2
  rep outsb

  // unlock the mutex
  lock dec byte ptr [mutex]

  # arg0 (rdi): processor ID    (u8)
  # arg1 (rsi): boot processor? (bool)
  # arg2 (rdx): multiboot info  (void *)
  # arg3 (rcx): number of cores (u8)
  # arg4  (r8): ???
  # arg5  (r9): ???
  mov rdi, r15
  cmp r15b, [bsp_id]
  setz sil
  mov edx, [multiboot_information]
  mov cl, [ncores]
halt:
  hlt
  jmp halt

.section ".data"
.extern multiboot_information
.globl bsp_id
.globl ncores

mutex: .byte 0
bsp_id: .byte 0
ncores: .byte 0
message_bsp: .string "Hello from BSP "
message_ap: .string "Hello from  AP "
message_end: .string "!\n"
base16: .string "0123456789ABCDEF"
