#include <cpuid.h>
#include <stdbool.h>

#include "loader.h"
#include "table.h"

void check_for_long_mode(void) {
  unsigned int eax = 0, ebx, ecx, edx = 0;
  if (!__get_cpuid(0x80000000, &eax, &ebx, &ecx, &edx)) {
    puts("ERROR (loader): CPUID failed\n");
    halt();
  }
  if (eax < 0x80000001) {
    puts("ERROR (loader): extended CPUID functions not available\n");
    halt();
  }
  if (!__get_cpuid(0x80000001, &eax, &ebx, &ecx, &edx)) {
    puts("ERROR (loader): CPUID failed\n");
    halt();
  }
  if ((edx & (1 << 29)) == 0) {
    puts("ERROR (loader): long mode not available\n");
    halt();
  }
}

struct pml4e pml4[512] __attribute__((aligned(4096)));
struct pdpe pdpt[512] __attribute__((aligned(4096)));

union gdte gdt[3];
struct gdtr gdtr __attribute__((aligned(4)));

void init_pdpt(void) {
  for (size_t i = 0; i < 512; i++) {
    pdpt[i].P_present = true;
    pdpt[i].RW_read_write = true;
    pdpt[i].PS_page_size = true; // 1GB pages
    pdpt[i].addr = i; // identity map
  }
}

void init_pml4(void) {
  unsigned long addr = (unsigned long)pdpt;
  // first 512GB
  pml4[0].P_present = true;
  pml4[0].RW_read_write = true;
  pml4[0].addr = addr >> 12;
  // first 512GB of higher half
  pml4[256] = pml4[0];
}

void init_gdt(void) {
  // null descriptor
  gdt[0].descriptor = (struct segment_descriptor){0};
  // code descriptor
  gdt[1].descriptor = (struct segment_descriptor) {
    .limit_0_15 = 0xFFFF,
    .limit_16_19 = 0xF,
    .base_0_23 = 0,
    .base_24_31 = 0,
    .access = (struct access_byte) {
      .P_present = true,
      .S_not_system = true,
      .E_executable = true,
      .RW_read_write = true,
    },
    .G_granularity = true,
    .DB_size_32 = false,
    .L_long_mode = true,
  };
  // data descriptor
  gdt[2].descriptor = (struct segment_descriptor) {
    .limit_0_15 = 0xFFFF,
    .limit_16_19 = 0xF,
    .base_0_23 = 0,
    .base_24_31 = 0,
    .access = (struct access_byte) {
      .P_present = true,
      .S_not_system = true,
      .E_executable = false,
      .RW_read_write = true,
    },
    .G_granularity = true,
    .DB_size_32 = true,
    .L_long_mode = false,
  };
  gdtr.limit = sizeof(gdt) - 1;
  gdtr.offset = (uint32_t)gdt;
}

void kmain(void) {
  check_for_long_mode();
  init_pdpt();
  init_pml4();
  init_gdt();

  unsigned nproc = acpi_nproc();
  extern uint8_t ncores;
  ncores = nproc;

  void *lapic = acpi_get_local_apic();
  volatile uint32_t *icr = lapic + 0x300;

  uint32_t eax, ebx, ecx, edx;
  cpuid(0x1, &eax, &ebx, &ecx, &edx);
  extern uint8_t bsp_id;
  bsp_id = (ebx >> 24);

  extern void trampoline(void);

  // send an INIT broadcast
  *icr = (
    5 << 8
    | 1 << 14
    | 3 << 18
  );
  // de-assert INIT
  while (*icr & (1 << 12)) {}
  *icr = (
    5 << 8
    | 1 << 15
    | 3 << 18
  );
  // send a SIPI with the trampoline page
  while (*icr & (1 << 12)) {}
  *icr = (
    (uint8_t)((uint32_t)trampoline/0x1000)
    | 6 << 8
    | 1 << 14
    | 3 << 18
  );
  while (*icr & (1 << 12)) {}

  void protected_mode(void);
  protected_mode();

  puts("ERROR (loader): still in loader???");
  halt();
}
