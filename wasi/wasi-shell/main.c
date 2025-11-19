#include "module.h"
#include "wasm-rt.h"


#include <arca/sys.h>
#include <assert.h>
#include <stdbool.h>

#define SELF_PAGE_TABLE 0

extern wasm_rt_memory_t *WASM_MEMORIES[128];
extern size_t WASM_MEMORIES_N;

static int len(const char *s) {
  int i = 0;
  while (s[i])
    i++;
  return i;
}

static void error_append(const char *msg) {
  arca_debug_log((const uint8_t *)msg, len(msg));
}

static void error_append_int(const char *msg, int value) {
  arca_debug_log_int((const uint8_t* )msg, len(msg), value);
}

[[noreturn]] void trap(const char *msg) {
  error_append(msg);
  arca_exit(0);
}

[[noreturn]] void abort(void) {
  error_append("abort");
  arca_exit(0);
}

uint64_t check(int64_t ret) {
  assert(ret >= 0);
  return ret;
}

int main(int argc, char **argv) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_wasi__snapshot__preview1 *)&module);
  w2c_module_0x5Fstart(&module);
  // TODO: set the return value correctly
  arca_exit(0);
}
