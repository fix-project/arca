#ifndef WASM_RT_EXCEPTIONS_H_
#define WASM_RT_EXCEPTIONS_H_
#include "wasm-rt.h"
typedef const void *wasm_rt_tag_t;
typedef fixcpp::Exception wasm_rt_exnref_t;
#define wasm_rt_exnref_null_value                                              \
  wasm_rt_exnref_t {}
#define WASM_RT_UNWIND_TARGET fixcpp::Target
extern "C" {
WASM_RT_NO_RETURN void wasm_rt_throw(void);
WASM_RT_UNWIND_TARGET *wasm_rt_get_unwind_target(void);
void wasm_rt_set_unwind_target(WASM_RT_UNWIND_TARGET *);
wasm_rt_tag_t wasm_rt_exception_tag(void);
}
#endif
