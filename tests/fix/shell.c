#include "fixture.h"
ENTRY {
  fix_handle scalar = fix_create_blob_i64(42);
  fix_handle fields[] = {scalar, fix_create_ref(scalar)};
  fix_handle saved = tree(fields, 2), copy[2];
  check(fix_access_tree(saved, 0, copy, 2) == 2);
  check(scalar == fields[0] && fix_equals(scalar, copy[0]));
  fix_close(copy[0]);
  fix_close(copy[1]);
  check(number(field(saved, 0)) == 42);
  fix_handle author = field(combination, 1);
  fix_handle tagged = fix_create_tag(tree(&author, 1));
  check(fix_is_tag(tagged));
  return fields[0];
}
