#include "fixture.h"
ENTRY {
  fix_handle argument = field(combination, 2), request = field(combination, 3);
  uint64_t mode = number(field(combination, 4));
  fix_handle fields[260];
  for (uint32_t i = 0; i < 260; i++)
    fields[i] = argument;
  fields[128] = blob("", 0);
  fields[129] = fields[128];
  fix_handle snapshot = tree(fields, 260);
  fix_handle closed = fix_clone(argument);
  fix_close(closed);
  fix_handle copy = fix_clone(argument);
  check(closed == copy);
  fix_close(copy);
  fix_close(fix_effect(request));
  check(fix_equals(field(snapshot, 127), argument));
  fix_close(fix_effect(request));
  check(fix_equals(field(snapshot, 259), argument));
  return mode == 2 ? snapshot : argument;
}
