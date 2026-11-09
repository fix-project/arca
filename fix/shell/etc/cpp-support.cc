#include "module.h"
#include "wasm-rt-exceptions.h"
#include "wasm-rt.h"
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <new>
extern "C" void *fix_cpp_malloc(std::size_t, std::size_t);
extern "C" void fix_cpp_free(void *);
extern "C" [[noreturn]] void fix_cpp_abort();
extern "C" void *malloc(std::size_t n) noexcept {
  return fix_cpp_malloc(n, 32);
}
extern "C" void free(void *p) noexcept { fix_cpp_free(p); }
extern "C" void *aligned_alloc(std::size_t a, std::size_t n) noexcept {
  return fix_cpp_malloc(n, a);
}
extern "C" void *calloc(std::size_t count, std::size_t n) noexcept {
  if (count && n > SIZE_MAX / count)
    return nullptr;
  void *p = malloc(count * n);
  if (p)
    std::memset(p, 0, count * n);
  return p;
}
extern "C" void *realloc(void *p, std::size_t n) noexcept {
  if (!p)
    return malloc(n);
  if (!n) {
    free(p);
    return nullptr;
  }
  auto *metadata = static_cast<std::size_t *>(p);
  auto offset = reinterpret_cast<std::uintptr_t>(p) - metadata[-3];
  auto old = metadata[-2] - offset;
  void *result = malloc(n);
  if (!result)
    return nullptr;
  std::memcpy(result, p, n < old ? n : old);
  free(p);
  return result;
}
extern "C" void abort() noexcept { fix_cpp_abort(); }
void *operator new(std::size_t n) {
  void *p = malloc(n);
  if (!p)
    throw std::bad_alloc();
  return p;
}
void *operator new[](std::size_t n) { return ::operator new(n); }
void operator delete(void *p) noexcept { free(p); }
void operator delete[](void *p) noexcept { free(p); }
void operator delete(void *p, std::size_t) noexcept { free(p); }
void operator delete[](void *p, std::size_t) noexcept { free(p); }
extern "C" void fix_cpp_init() {
  extern void (*__init_array_start[])();
  extern void (*__init_array_end[])();
  for (auto f = __init_array_start; f != __init_array_end; ++f)
    (*f)();
}
extern "C" {
void *__dso_handle = &__dso_handle;
size_t wasm_rt_module_size() { return sizeof(w2c_module); }
size_t wasm_rt_module_alignment() { return alignof(w2c_module); }
void cell_copy(fixcpp::Cell *d, const fixcpp::Cell *s) noexcept {
  wasm_rt_ref_assign(d, s);
}
void cell_clear(fixcpp::Cell *d) noexcept { wasm_rt_ref_assign(d, nullptr); }
unsigned cell_equal(const fixcpp::Cell *a, const fixcpp::Cell *b) noexcept {
  unsigned equal;
  __asm__ volatile(
      "xor %%eax, %%eax; mov 0(%1), %%r10; cmp 0(%2), %%r10; jne 1f;"
      "mov 8(%1), %%r10; cmp 8(%2), %%r10; jne 1f;"
      "mov 16(%1), %%r10; cmp 16(%2), %%r10; jne 1f;"
      "mov 24(%1), %%r10; cmp 24(%2), %%r10; jne 1f;"
      "mov $1, %%eax; 1: xor %%r10d, %%r10d"
      : "=&a"(equal)
      : "r"(a), "r"(b)
      : "r10", "memory", "cc");
  return equal;
}
}
namespace fixcpp {
static constexpr size_t capacity = (size_t(1) << 31) / sizeof(Cell);
static uint64_t occupied[capacity / 64];
static size_t first_free;
static bool used(size_t i) {
  return occupied[i / 64] & (uint64_t(1) << (i % 64));
}
static void mark(size_t i) { occupied[i / 64] |= uint64_t(1) << (i % 64); }
Cell *allocate() noexcept {
  auto count = size_t(wasm_rt_refs.top - wasm_rt_refs.base);
  while (first_free < count && used(first_free))
    ++first_free;
  if (first_free == count)
    return wasm_rt_ref_reserve(1);
  auto i = first_free++;
  mark(i);
  return wasm_rt_refs.base + i;
}
void release(Cell *p) noexcept {
  if (!p)
    return;
  auto i = size_t(p - wasm_rt_refs.base);
  assert(used(i));
  cell_clear(p);
  occupied[i / 64] &= ~(uint64_t(1) << (i % 64));
  if (i < first_free)
    first_free = i;
  auto count = size_t(wasm_rt_refs.top - wasm_rt_refs.base);
  while (count && !used(count - 1))
    --count;
  wasm_rt_refs.top = wasm_rt_refs.base + count;
}
[[noreturn]] void null_exception() { wasm_rt_trap(WASM_RT_TRAP_NULL_REF); }
} // namespace fixcpp
extern "C" void fix_cpp_mark(wasm_rt_ref_value *p, size_t n) {
  auto begin = size_t(p - wasm_rt_refs.base);
  for (size_t i = 0; i < n; ++i)
    fixcpp::mark(begin + i);
}
extern "C" void wasm_rt_throw() {
  if (!fixcpp::target)
    wasm_rt_trap(WASM_RT_TRAP_UNCAUGHT_EXCEPTION);
  throw fixcpp::Thrown{fixcpp::target, fixcpp::current};
}
extern "C" WASM_RT_UNWIND_TARGET *wasm_rt_get_unwind_target() {
  return fixcpp::target;
}
extern "C" void wasm_rt_set_unwind_target(WASM_RT_UNWIND_TARGET *p) {
  fixcpp::target = p;
}
extern "C" wasm_rt_tag_t wasm_rt_exception_tag() { return fixcpp::current.tag; }

struct w2c_fix;
template <class Function>
static void instantiate(Function function, w2c_module *module,
                        w2c_fix *context) {
  if constexpr (std::is_invocable_v<Function, w2c_module *, w2c_fix *>)
    function(module, context);
  else
    function(module);
}
extern "C" void
instantiate_bridge(w2c_module *,
                   w2c_fix *) __asm__("wasm2c_module_instantiate");
extern "C" void instantiate_bridge(w2c_module *module, w2c_fix *context) {
  new (module) w2c_module;
  instantiate(cpp_wasm2c_module_instantiate, module, context);
}
extern "C" void free_bridge(w2c_module *) __asm__("wasm2c_module_free");
extern "C" void free_bridge(w2c_module *module) {
  cpp_wasm2c_module_free(module);
  module->~w2c_module();
}
extern "C" void
apply_bridge(w2c_module *, fixcpp::Cell *,
             fixcpp::Cell *) __asm__("w2c_module_0x5Ffix_apply");
extern "C" void apply_bridge(w2c_module *module, fixcpp::Cell *result,
                             fixcpp::Cell *argument) {
  auto value =
      cpp_w2c_module_0x5Ffix_apply(module, fixcpp::Ref::from_cell(argument));
  cell_copy(result, value.slot());
}
extern "C" uint32_t
grow_raw(wasm_rt_externref_table_t *, uint32_t,
         fixcpp::Cell *) __asm__("wasm_rt_grow_externref_table");
extern "C" uint32_t
cpp_wasm_rt_grow_externref_table(wasm_rt_externref_table_t *table, uint32_t n,
                                 fixcpp::Ref init) {
  return grow_raw(table, n, init.slot());
}
