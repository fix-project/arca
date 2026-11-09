#include <assert.h>
#include <stdint.h>
#include <string.h>

#define memcpy test_memcpy
#define memmove test_memmove
#define memset test_memset
#include "fix/shell/inc/wasm-rt-memory.h"
#undef memcpy
#undef memmove
#undef memset

static unsigned char expected[16384], actual[16384], source[16384];

int main(void) {
  const size_t lengths[] = {0, 1, 17, 32, 64, 513, 4097, 8192};
  const size_t offsets[] = {0, 1, 13, 31};
  const int shifts[] = {-17, 0, 17};
  for (size_t i = 0; i < sizeof(source); i++)
    source[i] = (unsigned char)(i * 29 + (i >> 3));

  for (size_t i = 0; i < sizeof(lengths) / sizeof(*lengths); i++) {
    for (size_t j = 0; j < sizeof(offsets) / sizeof(*offsets); j++) {
      size_t n = lengths[i], offset = offsets[j];
      memcpy(expected, source, sizeof(source));
      memcpy(actual, source, sizeof(source));
      memcpy(expected + offset, source + 63 - offset, n);
      assert(test_memcpy(actual + offset, source + 63 - offset, n) ==
             actual + offset);
      assert(memcmp(expected, actual, sizeof(actual)) == 0);
      memset(expected + offset, 0x1a5, n);
      assert(test_memset(actual + offset, 0x1a5, n) == actual + offset);
      assert(memcmp(expected, actual, sizeof(actual)) == 0);
      for (size_t k = 0; k < sizeof(shifts) / sizeof(*shifts); k++) {
        size_t src = 128 + offset, dst = src + shifts[k];
        memcpy(expected, source, sizeof(source));
        memcpy(actual, source, sizeof(source));
        memmove(expected + dst, expected + src, n);
        assert(test_memmove(actual + dst, actual + src, n) == actual + dst);
        assert(memcmp(expected, actual, sizeof(actual)) == 0);
      }
    }
  }
  return 0;
}
