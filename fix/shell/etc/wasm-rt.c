#include "wasm-rt.h"
#include "wasm-rt-memory.h"

float nearbyintf(float value) { return __builtin_nearbyintf(value); }

double nearbyint(double value) { return __builtin_nearbyint(value); }

extern void fix_cpp_init(void);
void wasm_rt_init(void) { fix_cpp_init(); }

bool wasm_rt_is_initialized(void) { return true; }

void wasm_rt_free(void) {}

wasm_rt_ref_stack wasm_rt_refs;

void wasm_rt_ref_assign(wasm_rt_ref_value *destination,
                        const wasm_rt_ref_value *source) {
  _Static_assert(sizeof(wasm_rt_ref_value) == 32, "reference value width");
  if (source) {
    __asm__ volatile("movq 0(%1), %%r10\n\tmovq %%r10, 0(%0)\n\t"
                     "movq 8(%1), %%r10\n\tmovq %%r10, 8(%0)\n\t"
                     "movq 16(%1), %%r10\n\tmovq %%r10, 16(%0)\n\t"
                     "movq 24(%1), %%r10\n\tmovq %%r10, 24(%0)\n\t"
                     "xor %%r10d, %%r10d"
                     :
                     : "r"(destination), "r"(source)
                     : "r10", "memory", "cc");
  } else {
    __asm__ volatile("movq $0, 0(%0)\n\tmovq $0, 8(%0)\n\t"
                     "movq $0, 16(%0)\n\tmovq $0, 24(%0)"
                     :
                     : "r"(destination)
                     : "memory");
  }
}

wasm_rt_ref_value *wasm_rt_ref_reserve(size_t count) {
  wasm_rt_ref_value *frame = wasm_rt_refs.top;
  if (count > (size_t)(wasm_rt_refs.limit - frame))
    wasm_rt_ref_overflow();
  wasm_rt_ref_value *end = frame + count;
  if (end > wasm_rt_refs.mapped)
    wasm_rt_ref_grow(frame, end);
  wasm_rt_refs.top = end;
  extern void fix_cpp_mark(wasm_rt_ref_value *, size_t);
  fix_cpp_mark(frame, count);
  return frame;
}
