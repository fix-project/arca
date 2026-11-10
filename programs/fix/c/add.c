#include "fix.h"
static uint64_t number(fix_handle value) {
  uint64_t result;
  if (fix_len(value) != 8 || fix_access_blob(value, 0, &result, 8) != 8)
    __builtin_trap();
  return result;
}
__attribute__((export_name("_fix_apply"))) fix_handle
apply(fix_handle combination) {
  fix_handle arguments[2];
  if (fix_access_tree(combination, 2, arguments, 2) != 2)
    __builtin_trap();
  uint64_t sum = number(arguments[0]) + number(arguments[1]);
  fix_close(arguments[0]);
  fix_close(arguments[1]);
  return fix_create_blob_i64(sum);
}
