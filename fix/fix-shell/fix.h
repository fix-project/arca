#pragma once

#include <arca/arca.h>

#include "handle.h"

typedef struct fix_handle {
  fix_type type;
  arcad d;
} fix_handle;

typedef struct arcad_pair {
  arcad first;
  arcad second;
} arcad_pair;

typedef struct w2c_fixpoint w2c_fixpoint;

arcad type_to_arcad(fix_type type);
fix_handle arcad_to_handle(arcad type, arcad data);
fix_handle arca_tuple_to_handle(arcad tuple);
arcad_pair handle_to_arcad(fix_handle handle);
arcad handle_to_arca_tuple(fix_handle handle);

long check(char *msg, long ret);
