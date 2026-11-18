#include "fixture.h"
ENTRY {
  fix_advise(combination, "fix::title", 10, "probe", 5);
  const uint64_t initial[] = {0, 2};
  const uint64_t completed[] = {2, 2};
  fix_advise(combination, "fix::progress", 13, initial, sizeof(initial));
  fix_advise(combination, "fix::progress", 13, completed, sizeof(completed));
  const uint8_t binary[] = {0, 255};
  fix_advise(combination, "sample::binary", 14, binary, 2);
  fix_handle fields[] = {blob("freshly constructed advice target contents", 42),
                         combination};
  fix_advise(fields[0], "sample::local", 13, "probe", 5);
  fix_advise(tree(fields, 2), "sample::local", 13, "probe", 5);
  return combination;
}
