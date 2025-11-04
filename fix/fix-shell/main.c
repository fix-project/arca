#include "module.h"
#include "wasm-rt.h"
#include "bindings.h"

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
  arca_log("Done instantiating");
  bytes32 out = arca_blob_to_handle(arca_argument());
  wasm_rt_externref_t argument = (wasm_rt_externref_t)u8x32_from_bytes32(out);
  wasm_rt_externref_t result = w2c_module_0x5Ffixpoint_apply(&module, argument);
  arca_exit(handle_to_arca_blob(bytes32_from_u8x32(result)));
}
