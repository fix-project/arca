#include "fixture.h"
ENTRY {
  (void)combination;
  uint8_t bytes[256];
  for (uint32_t i = 0; i < 256; i++)
    bytes[i] = 7;
  fix_handle leaf = blob(bytes, 64), alias = fix_create_ref(leaf);
  fix_handle fields[] = {
      leaf, alias, tree(0, 0),
      fix_create_strict_encode(fix_create_identification_thunk(leaf))};
  fix_handle request = tree(fields, 4);
  for (uint32_t i = 0; i < 128; i++)
    bytes[i] = 9;
  fix_handle hidden_blob = blob(bytes, 128);
  fix_handle hidden[] = {hidden_blob, request};
  fix_handle hidden_tree = tree(hidden, 2);
  (void)blob(bytes, 256);
  fix_close(fix_effect(request));
  fix_close(fix_effect(alias));
  fix_handle result[] = {hidden_tree, hidden_blob, request};
  return tree(result, 3);
}
