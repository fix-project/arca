#include "fixture.h"
ENTRY {
  fix_handle prefix[126];
  for (uint32_t i = 0; i < 126; ++i)
    prefix[i] = fix_clone(combination);
  fix_handle suffix[3];
  check(fix_access_tree(combination, 2, suffix, 3) == 2);
  for (uint32_t i = 0; i < 126; ++i)
    fix_close(prefix[i]);
  fix_handle author = field(combination, 1);
  check(number(suffix[0]) == 17 && number(suffix[1]) == 23);
  check(fix_access_tree(combination, 4, suffix + 2, 1) == 0);
  fix_close(suffix[0]);
  fix_close(suffix[1]);

  fix_handle scalar = fix_create_blob_i64(42);
  fix_handle fields[] = {scalar, fix_create_ref(scalar)};
  fix_handle saved = tree(fields, 2), copy[2];
  check(fix_access_tree(saved, 0, copy, 2) == 2);
  check(scalar == fields[0] && fix_equals(scalar, copy[0]));
  fix_close(copy[0]);
  fix_close(copy[1]);
  check(number(field(saved, 0)) == 42);
  fix_handle tagged = fix_create_tag(tree(&author, 1));
  check(fix_is_tag(tagged));
  return fields[0];
}
