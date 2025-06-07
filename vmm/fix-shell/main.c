#include "module.h"
#include "wasm-rt.h"

#include "syscall.h"

#include <stdbool.h>

static int len(const char *s) {
  int i = 0;
  while (s[i])
    i++;
  return i;
}

static void error_append(const char *msg) {
  arca_error_append((const uint8_t *)msg, len(msg));
}

[[noreturn]] void abort(void) {
  arca_error_reset();
  error_append("abort");
  arca_error_return();
}

void __assert_fail(const char *assertion, const char *file, unsigned int line,
                   const char *function) {
  arca_error_reset();
  error_append("assertion failed: ");
  error_append(assertion);
  error_append(" at ");
  error_append(file);
  error_append(":");
  arca_error_append_int(line);
  error_append(" in ");
  error_append(function);
  arca_error_return();
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i32(struct w2c_fixpoint *instance,
                                                 uint32_t val) {
  return arca_word_create(val);
}

[[noreturn]] void fmain(void) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_fixpoint *)&module);
  wasm_rt_externref_t argument = arca_return_continuation_lambda();
  wasm_rt_externref_t result = w2c_module_0x5Ffixpoint_apply(&module, argument);
  arca_exit(result);
}
