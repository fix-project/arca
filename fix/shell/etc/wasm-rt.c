#include "wasm-rt.h"
#include "wasm-rt-exceptions.h"
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

size_t wasm_rt_module_alignment(void) {
  return _Alignof(w2c_module);
}

static wasm_rt_tag_t exception_tag;
static uint32_t exception_size;
static void *exception_payload;
static WASM_RT_UNWIND_TARGET *unwind_target;

extern void *wasm_rt_store_exception(uint32_t size, const void *values);

void wasm_rt_load_exception(wasm_rt_tag_t tag, uint32_t size, const void *values) {
  void *payload = wasm_rt_store_exception(size, values);
  if (!payload) {
    wasm_rt_trap(WASM_RT_TRAP_EXHAUSTION);
  }
  exception_tag = tag;
  exception_size = size;
  exception_payload = payload;
}

WASM_RT_NO_RETURN void wasm_rt_throw(void) {
  if (!unwind_target) {
    wasm_rt_trap(WASM_RT_TRAP_UNCAUGHT_EXCEPTION);
  }
  __builtin_longjmp(unwind_target->state, 1);
}

WASM_RT_UNWIND_TARGET *wasm_rt_get_unwind_target(void) {
  return unwind_target;
}

void wasm_rt_set_unwind_target(WASM_RT_UNWIND_TARGET *target) {
  unwind_target = target;
}

wasm_rt_tag_t wasm_rt_exception_tag(void) {
  return exception_tag;
}

uint32_t wasm_rt_exception_size(void) {
  return exception_size;
}

void *wasm_rt_exception(void) {
  return exception_payload;
}
