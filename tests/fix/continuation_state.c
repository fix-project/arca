#include "fixture.h"
static fix_handle remembered;
ENTRY {
  fix_handle argument = field(combination, 2), request = field(combination, 3);
  uint64_t mode = number(field(combination, 4));
  remembered = argument;
  uint8_t bytes[128] = {0};
  fix_handle modified = 0;
  if (mode == 1) {
    check(fix_access_blob(argument, 0, bytes, 64) == 64);
    bytes[63] = 99;
    modified = blob(bytes, 128);
  }
  fix_handle identification = fix_create_identification_thunk(argument);
  fix_handle fields[] = {argument, fix_create_ref(argument), identification,
                         fix_create_strict_encode(identification),
                         fix_create_shallow_encode(identification)};
  fix_handle saved = tree(fields, 5);
  fix_handle hole = fix_create_blob_i64(17);
  fix_close(hole);
  fix_handle reused = fix_clone(argument);
  check(hole == reused);
  fix_close(reused);
  for (uint32_t i = 0; i < 2; i++) {
    fix_handle reply = mode == 4 ? fix_return_cc(request) : fix_effect(request);
    if (mode == 3)
      check(fix_equals(reply, argument));
    fix_close(reply);
    check(fix_equals(remembered, argument));
    check(fix_equals(field(saved, 0), argument));
    check(fix_get_type(field(saved, 1)) == FIX_REF);
    check(fix_get_type(field(saved, 2)) == FIX_THUNK);
    if (mode == 1) {
      check(fix_access_blob(modified, 63, bytes, 1) == 1 && bytes[0] == 99);
      check(fix_access_blob(argument, 63, bytes, 1) == 1 && bytes[0] == 42);
    }
  }
  return remembered;
}
