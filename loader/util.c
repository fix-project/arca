#include "loader.h"

void outb(uint16_t port, uint8_t byte) {
  asm volatile ("outb %0, %1" :: "a"(byte), "d"(port));
}

void outw(uint16_t port, uint16_t byte) {
  asm volatile ("outw %0, %1" :: "a"(byte), "d"(port));
}

uint8_t inb(uint16_t port) {
  uint8_t byte;
  asm volatile ("inb %1, %0" : "=a"(byte) : "d"(port));
  return byte;
}

void cpuid(uint32_t leaf, uint32_t *eax, uint32_t *ebx, uint32_t *ecx, uint32_t *edx) {
  asm volatile ("cpuid" : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx) : "a"(leaf));
}

uint64_t cr0_get(void) {
  uint64_t ret;
  asm("mov %%cr0, %0" : "=r"(ret));
  return ret;
}

void cr0_set(uint64_t val) { asm("mov %0, %%cr0" ::"r"(val)); }

uint64_t cr1_get(void) {
  uint64_t ret;
  asm("mov %%cr1, %0" : "=r"(ret));
  return ret;
}

void cr1_set(uint64_t val) { asm("mov %0, %%cr1" ::"r"(val)); }

uint64_t cr2_get(void) {
  uint64_t ret;
  asm("mov %%cr2, %0" : "=r"(ret));
  return ret;
}

void cr2_set(uint64_t val) { asm("mov %0, %%cr2" ::"r"(val)); }

uint64_t cr3_get(void) {
  uint64_t ret;
  asm("mov %%cr3, %0" : "=r"(ret));
  return ret;
}

void cr3_set(uint64_t val) { asm("mov %0, %%cr3" ::"r"(val)); }

uint64_t cr4_get(void) {
  uint64_t ret;
  asm("mov %%cr4, %0" : "=r"(ret));
  return ret;
}

void cr4_set(uint64_t val) { asm("mov %0, %%cr4" ::"r"(val)); }

uint64_t msr_get(uint32_t msr) {
  uint64_t val;
  asm("rdmsr" : "=A"(val) : "c"(msr));
  return val;
}

void msr_set(uint32_t msr, uint64_t val) {
  asm("wrmsr" : : "c"(msr), "A"(val));
}
