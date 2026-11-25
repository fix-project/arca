#include "fixture.h"

WASM_MEMORY_COPY(1, 0);

ENTRY {
  if (fix_len(combination) == 3) {
    fix_handle resume[] = {field(combination, 0), field(combination, 2),
                           fix_create_blob_i64(42)};
    return fix_create_application_thunk(tree(resume, 3));
  }
  uint64_t mode = number(field(combination, 2));
  if (mode == 7 || mode == 8) {
    fix_handle input = field(combination, 3);
    check(fix_attach_blob(1, input) == fix_len(input));
    fix_close(input);
    if (mode == 7) {
      fix_close(fix_return_cc(fix_create_blob_i64(42)));
    } else {
      fix_detach_blob(1, 1);
    }
    uint8_t value = 0;
    wasm_memory_copy_1_to_0(&value, 0, 1);
    fix_detach_blob(1, 0);
    return fix_create_blob_i64(value);
  }
  uint32_t count = number(field(combination, 3));
  if (mode == 14 || mode == 15) {
    fix_handle callee = mode == 15 ? field(combination, 1) : 0;
    for (uint32_t i = 0; i < count; ++i) {
      if (mode == 14) {
        fix_handle value = fix_create_blob_i64(i);
        fix_handle evaluated = fix_eval(value);
        check(number(evaluated) == i);
        fix_close(evaluated);
        fix_close(value);
      } else {
        fix_handle result = fix_call_cc(callee);
        check(number(result) == 42);
        fix_close(result);
      }
    }
    if (callee)
      fix_close(callee);
    return fix_create_blob_i64(42);
  }
  if (mode >= 9 && mode <= 13) {
    check(wasm_memory_grow_1(count) != -1);
    for (uint32_t i = 0; i < 8; ++i) {
      uint8_t value = (uint8_t)i;
      for (uint32_t offset = 0; offset < count * 65536; offset += 4096)
        wasm_memory_copy_0_to_1((void *)(uintptr_t)offset, &value, 1);
      fix_handle object = fix_create_blob(1, count * 65536);
      if (mode == 13) {
        check(fix_attach_blob(1, object) == count * 65536);
        fix_close(object);
        uint8_t observed = 0;
        wasm_memory_copy_1_to_0(&observed, 0, 1);
        check(observed == value);
        fix_detach_blob(1, 0);
        check(wasm_memory_grow_1(count) != -1);
        continue;
      }
      fix_handle alias = mode == 9 ? fix_clone(object) : tree(&object, 1);
      fix_close(object);
      if (mode == 12) {
        object = field(alias, 0);
        fix_close(alias);
        alias = object;
        uint8_t observed = 0;
        check(fix_access_blob(alias, 0, &observed, 1) == 1 && observed == value);
      }
      if (mode != 10)
        fix_close(alias);
    }
    return fix_create_blob_i64(42);
  }
  if (mode == 6)
    return fix_call_cc(field(combination, 1));
  if (mode == 0 || mode == 4 || mode == 5) {
    check(wasm_memory_grow_1(count) != -1);
    if (mode == 4)
      fix_advise(combination, "memory::held", 12, "", 0);
    if (mode == 5)
      fix_close(fix_return_cc(fix_create_blob_i64(42)));
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
