#include "fix.h"
__attribute__((export_name("_fix_apply"))) fix_handle
apply(fix_handle combination) {
  fix_handle size_handle = fix_get_tree_element(combination, 2);
  uint64_t size;
  if (fix_access_blob(size_handle, 0, &size, 8) != 8)
    __builtin_trap();
  fix_close(size_handle);
  fix_handle request = fix_get_tree_element(combination, 4), last = request;
  for (uint64_t i = 0; i < size / 32; i++)
    last = fix_clone(request);
  for (;;) {
    if (!fix_equals(last, request))
      __builtin_trap();
    fix_close(fix_effect(request));
  }
}
