#include "fixture.h"
ENTRY {
  (void)combination;
  fix_handle fields[] = {fix_create_blob_i64(42), 0};
  fields[1] = fields[0];
  fix_handle inner = tree(fields, 2);
  fields[0] = inner;
  fields[1] = fix_create_blob_i64(7);
  return tree(fields, 2);
}
