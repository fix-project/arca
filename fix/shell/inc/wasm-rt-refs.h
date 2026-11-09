#ifndef WASM_RT_REFS_H_
#define WASM_RT_REFS_H_
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
typedef fixcpp::Cell wasm_rt_ref_value;
#define WASM_RT_REF_NOEXCEPT noexcept
#else
typedef struct {
  unsigned char bytes[32];
} wasm_rt_ref_value;
#define WASM_RT_REF_NOEXCEPT
#endif

typedef struct {
  wasm_rt_ref_value *base;
  wasm_rt_ref_value *top;
  wasm_rt_ref_value *limit;
  wasm_rt_ref_value *mapped;
} wasm_rt_ref_stack;

extern wasm_rt_ref_stack wasm_rt_refs;
void wasm_rt_ref_assign(wasm_rt_ref_value *,
                        const wasm_rt_ref_value *) WASM_RT_REF_NOEXCEPT;
wasm_rt_ref_value *wasm_rt_ref_reserve(size_t) WASM_RT_REF_NOEXCEPT;
void wasm_rt_ref_grow(wasm_rt_ref_value *,
                      wasm_rt_ref_value *) WASM_RT_REF_NOEXCEPT;
__attribute__((noreturn)) void wasm_rt_ref_overflow(void) WASM_RT_REF_NOEXCEPT;
#undef WASM_RT_REF_NOEXCEPT
#ifdef __cplusplus
}
#endif
#endif
