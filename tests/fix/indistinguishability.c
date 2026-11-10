#include "fixture.h"
ENTRY {
  fix_handle argument = field(combination, 2);
  if (fix_get_type(argument) == FIX_THUNK) {
    check(!fix_is_comparable(argument));
    fix_handle fields[] = {argument, fix_create_strict_encode(argument),
                           fix_create_shallow_encode(argument)};
    check(fix_get_encode_type(fields[1]) == FIX_STRICT &&
          fix_get_encode_type(fields[2]) == FIX_SHALLOW);
    (void)tree(fields, 3);
    fix_close(fix_effect(field(combination, 3)));
  }
  return argument;
}
