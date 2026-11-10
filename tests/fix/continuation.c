#include "fixture.h"
static uint64_t counter;
ENTRY {
  fix_handle program = field(combination, 1);
  fix_handle argument = field(combination, 2);
  if (fix_get_data_type(argument) == FIX_TREE) {
    fix_handle values[] = {field(combination, 0), argument,
                           fix_create_blob_i64(41)};
    return fix_create_application_thunk(tree(values, 3));
  }
  uint64_t mode = number(argument);
  fix_handle saved = fix_create_blob_i64(456);
  fix_handle fields[] = {saved};
  fix_handle graph = tree(fields, 1);
  fix_handle application = fix_create_application_thunk(graph);
  fix_handle objects[] = {graph,
                          fix_create_ref(graph),
                          application,
                          fix_create_strict_encode(application),
                          fix_create_shallow_encode(application),
                          fix_create_identification_thunk(saved),
                          fix_create_selection_thunk(graph)};
  graph = tree(objects, 7);
  check(!fix_is_comparable(graph));
  fix_handle reply = mode == 1 ? fix_call_cc(program)
                               : fix_return_cc(fix_create_blob_i64(123));
  ++counter;
  check(number(argument) == mode);
  for (uint32_t i = 0; i < 7; i++) {
    fix_handle item = field(graph, i);
    check(fix_get_type(item) == (i == 0             ? FIX_OBJECT
                                 : i == 1           ? FIX_REF
                                 : i == 3 || i == 4 ? FIX_ENCODE
                                                    : FIX_THUNK));
    fix_close(item);
  }
  check(number(field(field(graph, 0), 0)) == 456);
  fields[0] = fix_create_blob_i64(457);
  check(number(saved) == 456 && number(fields[0]) == 457);
  if (mode == 2) {
    reply = fix_return_cc(reply);
    ++counter;
  }
  if (mode >= 3) {
    fix_handle author = combination;
    for (uint64_t depth = 3; depth < mode; depth++)
      author = field(author, 1);
    return fix_create_tag(tree(&author, 1));
  }
  return fix_create_blob_i64(number(reply) + counter);
}
