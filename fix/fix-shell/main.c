#include "fix.h"
#include "module.h"
#include "wasm-rt.h"

#include <arca/arca.h>
#include <arca/sys.h>
#include <assert.h>
#include <stdbool.h>

#define SELF_PAGE_TABLE 0

[[noreturn]] void trap(const char *msg) { arca_panic(msg); }

[[noreturn]] void abort(void) { arca_panic("abort"); }

[[noreturn]] void fmain(void) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_fixpoint *)&module);
  wasm_rt_externref_t argument = arca_tuple_to_handle(arca_argument());
  wasm_rt_externref_t result = w2c_module_0x5Ffixpoint_apply(&module, argument);
  arca_exit(handle_to_arca_tuple(result));
}
