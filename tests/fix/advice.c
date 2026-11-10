#include "fixture.h"
ENTRY {
  fix_advise(combination, "fix::title", 10, "probe", 5);
  fix_advise(combination, "fix::progress", 13, "first", 5);
  fix_advise(combination, "fix::progress", 13, "last", 4);
  const uint8_t binary[] = {0, 255};
  fix_advise(combination, "sample::binary", 14, binary, 2);
  fix_handle fields[] = {blob("freshly constructed advice target contents", 42),
                         combination};
  fix_advise(fields[0], "sample::local", 13, "probe", 5);
  fix_advise(tree(fields, 2), "sample::local", 13, "probe", 5);
  return combination;
}
