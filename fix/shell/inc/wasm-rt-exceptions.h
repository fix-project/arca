#ifndef WASM_RT_EXCEPTIONS_H_
#define WASM_RT_EXCEPTIONS_H_
#include "wasm-rt.h"
typedef const void *wasm_rt_tag_t;
typedef void *wasm_rt_jmp_buf[5];
#define WASM_RT_UNWIND_TARGET wasm_rt_jmp_buf
#define wasm_rt_try(target) __builtin_setjmp((void **)(target))
#define WASM_EXN_MAX_SIZE 256
typedef struct {
  wasm_rt_tag_t tag;
  uint32_t size;
  char data[WASM_EXN_MAX_SIZE];
} wasm_rt_exnref_t;
#define wasm_rt_exnref_null_value ((wasm_rt_exnref_t){0})
void wasm_rt_load_exception(wasm_rt_tag_t, uint32_t, const void *);
WASM_RT_NO_RETURN void wasm_rt_throw(void);
WASM_RT_UNWIND_TARGET *wasm_rt_get_unwind_target(void);
void wasm_rt_set_unwind_target(WASM_RT_UNWIND_TARGET *);
wasm_rt_tag_t wasm_rt_exception_tag(void);
uint32_t wasm_rt_exception_size(void);
void *wasm_rt_exception(void);
#endif
