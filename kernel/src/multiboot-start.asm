bits 32

global multiboot_header
global _multiboot
global common_boot
extern _bsp_start
extern _ap_start

%define ka2pa(x) (x - start + 0x100000)

section .multiboot
start:

%define MB_MAGIC 0x1BADB002
%define MBF_ALIGN_MODULES (1 << 0)
%define MBF_MEMORY_INFO   (1 << 1)
%define MBF_VIDEO_INFO    (1 << 2)
%define MBF_NOTELF        (1 << 16)

%define MB_FLAGS MBF_MEMORY_INFO
align 32
multiboot_header:
mbh_magic: dd MB_MAGIC
mbh_flags: dd MB_FLAGS
mbh_cksum: dd -(MB_MAGIC + MB_FLAGS)

; pml4: 0xa000
; pdpt: 0xb000

_multiboot:
  cmp eax, 0x2BADB002
  jne multiboot_error

  ; clear page tables
  mov edi, 0x1000
  mov ecx, (4096 * 2 / 4)
  xor eax, eax
  rep stosd

  ; point pml4 at pdpt
  mov edi, 0x1000
  mov dword [edi], 0x2003
  mov dword [edi + 8*256], 0x2003

  ; id map pdpt
  mov edi, 0x2000
  mov ecx, 512
.init_pdpt:
  mov eax, ecx
  shl eax, 30
  or eax, 0x83
  mov edx, ecx
  shr edx, 2
  mov dword [edi + 8*ecx], eax
  mov dword [edi + 8*ecx + 4], edx
  loop .init_pdpt
  mov dword [0x2000], 0x00000083

common_boot:
  ; PAE
  mov eax, (1 << 5)
  mov cr4, eax

  ; activate page table
  mov edi, 0x1000
  mov cr3, edi

  ; enable long mode
  mov ecx, 0xC0000080
  rdmsr
  or eax, 1 << 8
  wrmsr

  ; enable paging
  mov eax, cr0
  or eax, 1 << 31
  mov cr0, eax

  ; load 64-bit gdt
  lgdt [ka2pa(gdtr)]
  jmp 0x8:(ka2pa(long_mode))

multiboot_success:
  mov al, 'S'
  mov dx, 0xe9
  out dx, al
  hlt

multiboot_error:
  mov al, 'E'
  mov dx, 0xe9
  out dx, al
  hlt

align 4
gdtr:
dw (gdt_end - gdt - 1)
dq gdt

align 8
gdt:
gdt_null:
  dq 0
gdt_code:
  dw 0xffff     ; limit
  dw 0x0000     ; base
  db 0x00       ; base
  db 0b10011010 ; access
  db 0b10101111 ; flags | limit
  db 0x00       ; base
gdt_data:
  dw 0xffff     ; limit
  dw 0x0000     ; base
  db 0x00       ; base
  db 0b10010010 ; access
  db 0b11001111 ; flags | limit
  db 0x00       ; base
gdt_end:

bits 64
long_mode:
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax

  mov rax, higher_half
  jmp rax

higher_half:
  cmp ebx, 0
  jne _bsp_start
  jmp _ap_start
  hlt

; --- trampoline for 16-bit code ---
bits 16

%define relocate(x) (x - trampoline_start + 0x8000)

global trampoline_start
global trampoline_end
section .text

trampoline_start:
trampoline:
  cli
  jmp real

align 4
gdtr32:
dw (gdt32_end - gdt32 - 1)
dd relocate(gdt32)

gdt32:
gdt32_null:
  dq 0
gdt32_code:
  dw 0xffff
  dw 0x0000
  db 0x0000
  db 0b10011011
  db 0b11001111
  db 0x00
gdt32_data:
  dw 0xffff
  dw 0x0000
  db 0x0000
  db 0b10010011
  db 0b11001111
  db 0x00
gdt32_end:

real:
  ; enable A20 line
  in al, 0x92
  or al, 2
  out 0x92, al

  ; enable protected mode
  mov eax, cr0
  or al, 1
  mov cr0, eax

  ; load 32-bit GDT
  lgdt [relocate(gdtr32)]

  ; reload selectors
  mov ax, 0x10
  mov ds, ax
  mov es, ax
  mov fs, ax
  mov gs, ax
  mov ss, ax
  jmp 0x08:relocate(protected)

bits 32
protected:
  mov ebx, 0
  mov eax, ka2pa(common_boot)
  jmp eax

trampoline_end:
