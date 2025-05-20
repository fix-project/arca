#include "module.h"
#include "wasm-rt.h"

#include <stdbool.h>

[[noreturn]] void abort(void) {
  for (;;) {
    asm("int3");
  }
}

static uint64_t capacity = 1;
static uint64_t assigned = 0;

void __assert_fail(const char * assertion, const char * file, unsigned int line, const char * function) {
  abort();
}

void putc(int c);

void puts(char *s) {
  while (*s) {
    putc(*s++);
  }
}

int64_t syscall(uint64_t num, ...);

int64_t resize(size_t len) {
  return syscall(0x04, len);
}

int64_t prompt(size_t dst) {
  return syscall(0x57, dst);
}

[[noreturn]] void arca_exit(size_t src) {
  while (true) {
    syscall(0x10, src);
    asm("ud2");
  }
}

uint64_t next_fd() {
  ++assigned;
  if (assigned >= capacity) {
    capacity *= 2;
    resize(capacity);
  }
  return assigned;
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i32(struct w2c_fixpoint *instance, uint32_t val) {
  uint64_t created = next_fd();
  syscall(0x60, created, val);
  return created;
}

[[noreturn]] void fmain(void) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_fixpoint *)&module);
  wasm_rt_externref_t argument = next_fd();
  prompt(argument);
  wasm_rt_externref_t result = w2c_module_0x5Ffixpoint_apply(&module, argument);
  arca_exit(result);
  puts("fixpoint\n");
  for(;;) {}
}
