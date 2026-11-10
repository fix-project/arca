#ifndef FIX_TEST_FIXTURE_H
#define FIX_TEST_FIXTURE_H
#include "fix.h"
#include "wasm.h"
#define ENTRY                                                                  \
  __attribute__((export_name("_fix_apply"))) fix_handle apply(                 \
      fix_handle combination)
WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
static inline void check(int condition) {
  if (!condition)
    __builtin_trap();
}
static inline fix_handle field(fix_handle tree, uint32_t index) {
  fix_handle value;
  check(fix_access_tree(tree, index, &value, 1) == 1);
  return value;
}
static inline uint64_t number(fix_handle value) {
  uint64_t out;
  check(fix_len(value) == sizeof(out));
  check(fix_access_blob(value, 0, &out, sizeof(out)) == sizeof(out));
  return out;
}
static inline fix_handle blob(const void *bytes, uint32_t length) {
  uint32_t pages = length / 65536 + (length % 65536 != 0);
  uint32_t current = wasm_memory_size_1();
  if (pages > current)
    check(wasm_memory_grow_1(pages - current) != -1);
  wasm_memory_copy_0_to_1(0, bytes, length);
  return fix_create_blob(1, length);
}
static inline fix_handle tree(const fix_handle *values, uint32_t length) {
  return fix_create_tree(values, length);
}
#endif
