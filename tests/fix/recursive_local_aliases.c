#include "fixture.h"
ENTRY {
  (void)combination;
  uint8_t bytes[64];
  for (uint32_t i = 0; i < 64; i++)
    bytes[i] = 41;
  fix_handle fields[5];
  fields[0] = blob(bytes, 64);
  fields[1] = fields[0];
  fields[2] = tree(fields, 2);
  fields[3] = fields[2];
  fields[4] = tree(fields, 4);
  return tree(fields, 5);
}
