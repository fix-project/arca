#include "fixture.h"

ENTRY {
  uint64_t mode = number(field(combination, 2));
  uint32_t count = number(field(combination, 3));
  if (mode == 0 || mode == 4) {
    check(wasm_memory_grow_1(count) != -1);
    if (mode == 4)
      fix_advise(combination, "memory::held", 12, "", 0);
  } else if (mode == 1) {
    check(wasm_memory_grow_1(16) != -1);
    for (uint32_t i = 0; i < count; i++) {
      fix_handle object = fix_create_blob(1, 1 << 20);
      check(fix_len(object) == 1 << 20);
    }
  } else if (mode == 3) {
    check(wasm_memory_grow_1(count) != -1);
    fix_detach_blob(1, 0);
    check(wasm_memory_grow_1(count) != -1);
  } else {
    for (uint32_t i = 0; i < count; i++)
      fix_create_blob_i64(i);
  }
  return fix_create_blob_i64(42);
}
