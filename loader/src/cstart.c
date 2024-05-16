#include "loader.h"

void kmain(void *);

void halt(void) {
  while (1) {
    asm volatile ("hlt");
  }
}

void _cstart(void *multiboot_info) {
  extern volatile char _sbss, _ebss;
  extern volatile char _strampoline, _etrampoline, _ltrampoline;

  volatile char *p = &_sbss;

  while (p < &_ebss) {
    *p++ = 0;
  }

  p = &_strampoline;
  volatile char *q = &_ltrampoline;

  while (p < &_etrampoline) {
    *p++ = *q++;
  }

  kmain(multiboot_info);
  puts("\r\nERROR: loader kmain exited!\n");

  halt();
}

