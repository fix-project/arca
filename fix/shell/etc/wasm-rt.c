#include "wasm-rt.h"
#include "fix-abi.h"
#include "module.h"
#include "wasm-rt-memory.h"

struct w2c_fix;
typedef void (*instantiate_empty)(w2c_module *);
typedef void (*instantiate_fix)(w2c_module *, struct w2c_fix *);
static void instantiate_without_context(instantiate_empty function,
                                        w2c_module *module,
                                        struct w2c_fix *fix) {
  (void)fix;
  function(module);
}
static void instantiate_with_context(instantiate_fix function,
                                     w2c_module *module, struct w2c_fix *fix) {
  function(module, fix);
}
size_t wasm_rt_module_size(void) { return sizeof(w2c_module); }
size_t wasm_rt_module_alignment(void) { return _Alignof(w2c_module); }
void wasm_rt_module_instantiate(w2c_module *module, struct w2c_fix *fix) {
  _Generic(&wasm2c_module_instantiate,
      instantiate_empty: instantiate_without_context,
      instantiate_fix: instantiate_with_context)(&wasm2c_module_instantiate,
                                                 module, fix);
}

float nearbyintf(float value) { return __builtin_nearbyintf(value); }

double nearbyint(double value) { return __builtin_nearbyint(value); }

void wasm_rt_init(void) {}
bool wasm_rt_is_initialized(void) { return true; }
void wasm_rt_free(void) {}

#include "wasm-rt-exceptions.h"
static WASM_RT_UNWIND_TARGET *unwind_target;
static wasm_rt_exnref_t exception;
void wasm_rt_load_exception(wasm_rt_tag_t tag, uint32_t size,
                            const void *values) {
  if (size > WASM_EXN_MAX_SIZE)
    wasm_rt_trap(WASM_RT_TRAP_UNCAUGHT_EXCEPTION);
  exception.tag = tag;
  exception.size = size;
  memcpy(exception.data, values, size);
}
void wasm_rt_throw(void) {
  if (!unwind_target)
    wasm_rt_trap(WASM_RT_TRAP_UNCAUGHT_EXCEPTION);
  __builtin_longjmp((void **)*unwind_target, 1);
}
WASM_RT_UNWIND_TARGET *wasm_rt_get_unwind_target(void) { return unwind_target; }
void wasm_rt_set_unwind_target(WASM_RT_UNWIND_TARGET *target) {
  unwind_target = target;
}
wasm_rt_tag_t wasm_rt_exception_tag(void) { return exception.tag; }
uint32_t wasm_rt_exception_size(void) { return exception.size; }
void *wasm_rt_exception(void) { return exception.data; }
