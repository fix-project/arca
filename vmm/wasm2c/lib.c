#include "module.h"
#include "wasm-rt.h"

#include "syscall.h"

#include <stdbool.h>

[[noreturn]] void abort(void) {
  for (;;) {
    asm("int3");
  }
}

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

int64_t prompt() {
  return syscall(SYS_RETURN_CONTINUATION_LAMBDA);
}

[[noreturn]] void arca_exit(size_t src) {
  while (true) {
    syscall(SYS_EXIT, src);
    asm("ud2");
  }
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i32(struct w2c_fixpoint *instance, uint32_t val) {
  return syscall(SYS_CREATE_WORD, val);
}

[[noreturn]] void fmain(void) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_fixpoint *)&module);
  wasm_rt_externref_t argument = prompt();
  wasm_rt_externref_t result = w2c_module_0x5Ffixpoint_apply(&module, argument);
  arca_exit(result);
  puts("fixpoint\n");
  for(;;) {}
}
