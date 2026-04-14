#include <assert.h>

#include "wasm-rt.h"
#include "module.h"

void wasm_rt_init(void) {
}

bool wasm_rt_is_initialized(void) {
  return true;
}

void wasm_rt_free(void) {
}

size_t wasm_rt_module_size(void) {
  return sizeof(w2c_module);
}
