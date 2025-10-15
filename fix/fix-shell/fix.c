#include "fix.h"
#include "arca/arca.h"
#include "arca/asm.h"
#include "arca/sys.h"

arcad type_to_arcad(fix_type type) {
  return arca_word_create((int64_t)(type));
}

static fix_type arcad_to_type(arcad type) {
  uint64_t word;
  arca_word_read(type, &word);
  if (word == BlobObject) {
    return BlobObject;
  }

  if (word == TreeObject) {
    return TreeObject;
  }

  return Null;
}

long check(char *msg, long ret) {
  if (ret >= 0) {
    return ret;
  }
  arca_panic(msg);
}

fix_handle arcad_to_handle(arcad type, arcad data) {
  fix_handle res = {.type = arcad_to_type(type), .d = data};
  return res;
}

fix_handle arca_tuple_to_handle(arcad tuple) {
  if (arca_type(tuple) != __TYPE_tuple) {
    arca_panic("arca_tuple_to_handle: input is not a tuple");
  }

  size_t len;
  check("arca_length", arca_length(tuple, &len));
  if (len != 2) {
    arca_panic("arca_tuple_to_handle: input is not a 2-entry tuple");
  }

  return arcad_to_handle(arca_tuple_get(tuple, 0), arca_tuple_get(tuple, 1));
}

arcad_pair handle_to_arcad(fix_handle handle) {
  arcad_pair res = {type_to_arcad(handle.type), handle.d};
  return res;
}

arcad handle_to_arca_tuple(fix_handle handle) {
  arcad tuple = arca_tuple_create(2);
  arcad_pair p = handle_to_arcad(handle);
  arca_tuple_set(tuple, 0, p.first);
  arca_tuple_set(tuple, 1, p.second);
  return tuple;
}
