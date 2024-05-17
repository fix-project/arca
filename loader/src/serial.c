#include "loader.h"

void putc(char c) { outb(0xe9, c); }

void puts(char *s) {
  while (*s) {
    putc(*s++);
  }
}

void putsn(char *s, size_t n) {
  for (size_t i = 0; i < n; i++) {
    putc(s[i]);
  }
}

void putx(unsigned x) {
  for (int i = 28; i >= 0; i -= 4) {
    unsigned y = (x >> i) & 0xf;
    putc("0123456789ABCDEF"[y]);
  }
}
