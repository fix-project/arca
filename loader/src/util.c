#include "loader.h"

void outb(uint16_t port, uint8_t byte) {
  asm volatile("outb %0, %1" ::"a"(byte), "d"(port));
}

void outw(uint16_t port, uint16_t byte) {
  asm volatile("outw %0, %1" ::"a"(byte), "d"(port));
}

uint8_t inb(uint16_t port) {
  uint8_t byte;
  asm volatile("inb %1, %0" : "=a"(byte) : "d"(port));
  return byte;
}

void cpuid(uint32_t leaf, uint32_t *eax, uint32_t *ebx, uint32_t *ecx,
           uint32_t *edx) {
  asm volatile("cpuid"
               : "=a"(eax), "=b"(ebx), "=c"(ecx), "=d"(edx)
               : "a"(leaf));
}
