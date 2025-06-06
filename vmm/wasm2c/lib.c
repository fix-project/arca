#include "module.h"
#include "wasm-rt.h"

#include "syscall.h"

#include <stdbool.h>

#define SELF_PAGE_TABLE 0

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

uint64_t check(int64_t ret) {
  if (ret < 0)
    abort();
  return ret;
}

uint64_t prompt() { return check(syscall(SYS_RETURN_CONTINUATION_LAMBDA)); }

[[noreturn]] void arca_exit(size_t src) {
  while (true) {
    syscall(SYS_EXIT, src);
    asm("ud2");
  }
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i32(struct w2c_fixpoint *instance, uint32_t val) {
  return check(syscall(SYS_CREATE_WORD, val));
}

void put_rw(uint64_t dst, uint64_t entry, size_t index) {
  check(syscall(SYS_PUT_RW, dst, entry, index));
}

void put_ro(uint64_t dst, uint64_t entry, size_t index) {
  check(syscall(SYS_PUT_RO, dst, entry, index));
}

uint64_t create_page() { return check(syscall(SYS_CREATE_PAGE, 1 << 12)); }

uint64_t create_table(size_t size) {
  return check(syscall(SYS_CREATE_TABLE, size));
}

void write(uint64_t dst, size_t offset, char *ptr, size_t size) {
  check(syscall(SYS_WRITE, dst, offset, ptr, size));
}

void read(uint64_t src, void *ptr, size_t size) {
  check(syscall(SYS_READ, src, ptr, size));
}

uint64_t take(uint64_t src, size_t index) {
  return check(syscall(SYS_TAKE, src, index));
}

size_t len(uint64_t src) {
  size_t n;
  check(syscall(SYS_LENGTH, src, &n));
  return n;
}

void map(uint64_t mapee, void *address) {
  check(syscall(SYS_MAP, SELF_PAGE_TABLE, (size_t)address >> 12, mapee));
}

void map_ro_page(uint64_t mapee, void *address) {
  check(syscall(SYS_MAP_RO, SELF_PAGE_TABLE, (size_t)address >> 12, mapee));
}

void map_rw_page(uint64_t mapee, void *address) {
  check(syscall(SYS_MAP_RW, SELF_PAGE_TABLE, (size_t)address >> 12, mapee));
}

enum datatype get_type(uint64_t v) { return check(syscall(SYS_TYPE, v)); }

wasm_rt_externref_t w2c_fixpoint_get_tree_entry(struct w2c_fixpoint *instance,
                                                uint64_t handle,
                                                uint32_t index) {
  return take(handle, index);
}

void w2c_fixpoint_attach_blob(struct w2c_fixpoint *instance, uint64_t handle,
                              uint32_t base_address) {
  void *addr = w2c_module_memory((w2c_module *)instance)->data + base_address;

  switch (get_type(handle)) {
  case DATATYPE_WORD: {
    uint64_t page = create_page();
    // XXX: map ro page
    read(handle, addr, 8);
    return;
  }

  case DATATYPE_BLOB: {
    // XXX: map ro page
    read(handle, addr, len(handle));
    return;
  }

  case DATATYPE_PAGE: {
    map_ro_page(handle, addr);
    return;
  }

  case DATATYPE_TABLE: {
    map(handle, addr);
    return;
  }

  default:
    abort();
  }

  return;
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
