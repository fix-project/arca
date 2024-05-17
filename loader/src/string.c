#include "loader.h"
#include <stddef.h>

int memcmp(const void *s1, const void *s2, size_t n) {
  const char *x = s1;
  const char *y = s2;
  for (size_t i = 0; i < n; i++) {
    if (x[i] < y[i])
      return -1;
    if (x[i] > y[i])
      return 1;
  }
  return 0;
}

void *memcpy(void *dest, const void *src, size_t n) {
  volatile char *x = dest;
  const char *y = src;
  for (size_t i = 0; i < n; i++) {
    x[i] = y[i];
  }
  return 0;
}
