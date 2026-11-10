#include "fixture.h"
ENTRY {
  (void)combination;
  fix_handle fields[5000];
  uint8_t bytes[64] = {0};
  for (uint32_t i = 0; i < 5000; i++) {
    for (uint32_t j = 0; j < 4; j++)
      bytes[j] = (uint8_t)(i >> (j * 8));
    fields[i] = blob(bytes, 64);
  }
  return tree(fields, 5000);
}
