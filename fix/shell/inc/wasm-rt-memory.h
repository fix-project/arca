#ifndef WASM_RT_MEMORY_H_
#define WASM_RT_MEMORY_H_

#include <stddef.h>
#include <stdint.h>

/* Compile freestanding: these loops must not become calls to themselves. */
void *memcpy(void *restrict destination, const void *restrict source,
             size_t n) {
  unsigned char *d = destination;
  const unsigned char *s = source;
  for (size_t i = 0; i < n; i++)
    d[i] = s[i];
  return destination;
}

void *memmove(void *destination, const void *source, size_t n) {
  unsigned char *d = destination;
  const unsigned char *s = source;
  if ((uintptr_t)d - (uintptr_t)s >= n) {
    for (size_t i = 0; i < n; i++)
      d[i] = s[i];
  } else {
    for (size_t i = n; i != 0; i--)
      d[i - 1] = s[i - 1];
  }
  return destination;
}

void *memset(void *destination, int value, size_t n) {
  unsigned char *d = destination;
  for (size_t i = 0; i < n; i++)
    d[i] = (unsigned char)value;
  return destination;
}

#endif
