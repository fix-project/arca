#include "fixture.h"
ENTRY {
  fix_handle saved = field(combination, 2);
  uint64_t mode = number(field(combination, 3));
  for (uint64_t i = 0; i < 64; i++) {
    fix_handle reply = fix_effect(saved);
    check(number(reply) == i);
    fix_close(reply);
  }
  if (mode == 1)
    return fix_unsound_handle_bytes(saved);
  if (mode == 2 || mode == 4)
    return fix_create_tag(tree(&combination, 1));
  return fix_create_blob_i64(64);
}
