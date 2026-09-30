#include "arca.h"
#pragma clang diagnostic ignored "-Wint-conversion"
#pragma clang diagnostic ignored "-Wunused-function"

static __inline long syscall0(long n) {
  unsigned long ret;
  __asm__ __volatile__("syscall" : "=a"(ret) : "a"(n) : "rcx", "r11", "memory");
  return ret;
}

static __inline long syscall1(long n, long a1) {
  unsigned long ret;
  __asm__ __volatile__("syscall"
                       : "=a"(ret)
                       : "a"(n), "D"(a1)
                       : "rcx", "r11", "memory");
  return ret;
}

static __inline long syscall2(long n, long a1, long a2) {
  unsigned long ret;
  __asm__ __volatile__("syscall"
                       : "=a"(ret)
                       : "a"(n), "D"(a1), "S"(a2)
                       : "rcx", "r11", "memory");
  return ret;
}

static __inline long syscall3(long n, long a1, long a2, long a3) {
  unsigned long ret;
  __asm__ __volatile__("syscall"
                       : "=a"(ret)
                       : "a"(n), "D"(a1), "S"(a2), "d"(a3)
                       : "rcx", "r11", "memory");
  return ret;
}

static __inline long syscall4(long n, long a1, long a2, long a3, long a4) {
  unsigned long ret;
  register long r10 __asm__("r10") = a4;
  __asm__ __volatile__("syscall"
                       : "=a"(ret)
                       : "a"(n), "D"(a1), "S"(a2), "d"(a3), "r"(r10)
                       : "rcx", "r11", "memory");
  return ret;
}

static __inline long syscall5(long n, long a1, long a2, long a3, long a4,
                              long a5) {
  unsigned long ret;
  register long r10 __asm__("r10") = a4;
  register long r8 __asm__("r8") = a5;
  __asm__ __volatile__("syscall"
                       : "=a"(ret)
                       : "a"(n), "D"(a1), "S"(a2), "d"(a3), "r"(r10), "r"(r8)
                       : "rcx", "r11", "memory");
  return ret;
}

static __inline long syscall6(long n, long a1, long a2, long a3, long a4,
                              long a5, long a6) {
  unsigned long ret;
  register long r10 __asm__("r10") = a4;
  register long r8 __asm__("r8") = a5;
  register long r9 __asm__("r9") = a6;
  __asm__ __volatile__("syscall"
                       : "=a"(ret)
                       : "a"(n), "D"(a1), "S"(a2), "d"(a3), "r"(r10), "r"(r8),
                         "r"(r9)
                       : "rcx", "r11", "memory");
  return ret;
}

void arca_nop(void) { syscall0(__NR_nop); }

arcad arca_clone(arcad value) { return syscall1(__NR_clone, value); }

int64_t arca_drop(arcad value) { return syscall1(__NR_drop, value); }

[[noreturn]] void arca_exit(arcad value) {
  syscall1(__NR_exit, value);
  for (;;)
    __asm__("ud2");
}

arcad arca_argument(void) { return syscall0(__NR_get_argument); }

int64_t arca_type(arcad value) { return syscall1(__NR_type, value); }

arcad arca_null_create(void) { return syscall0(__NR_create_null); }

arcad arca_word_create(uint64_t value) {
  return syscall1(__NR_create_word, value);
}

arcad arca_blob_create(const uint8_t *data, size_t len) {
  return syscall2(__NR_create_blob, data, len);
}

arcad arca_tuple_create(size_t len) { return syscall1(__NR_create_tree, len); }

arcad arca_page_create(size_t size) { return syscall1(__NR_create_page, size); }

arcad arca_table_create(size_t size) {
  return syscall1(__NR_create_table, size);
}

arcad arca_function_create(arcad data) {
  return syscall1(__NR_create_function, data);
}

arcad arca_word_read(arcad word, uint64_t *output) {
  return syscall2(__NR_read, word, output);
}

arcad arca_blob_read(arcad blob, size_t offset, uint8_t *data, size_t len) {
  return syscall4(__NR_read, blob, offset, data, len);
}

arcad arca_page_read(arcad page, size_t offset, uint8_t *data, size_t len) {
  return syscall4(__NR_read, page, offset, data, len);
}

arcad arca_function_read(arcad function) {
  return syscall1(__NR_read, function);
}

arcad arca_blob_write(arcad blob, size_t offset, const uint8_t *data,
                      size_t len) {
  return syscall4(__NR_write, blob, offset, data, len);
}

arcad arca_page_write(arcad page, size_t offset, const uint8_t *data,
                      size_t len) {
  return syscall4(__NR_write, page, offset, data, len);
}

int64_t arca_equals(arcad x, arcad y) { return syscall2(__NR_equals, x, y); }

int64_t arca_length(arcad value, size_t *output) {
  return syscall2(__NR_length, value, output);
}

arcad arca_tuple_get(arcad value, size_t index) {
  return syscall2(__NR_get, value, index);
}

arcad arca_table_get(arcad table, size_t index, struct arca_entry *entry) {
  return syscall3(__NR_get, table, index, entry);
}

int64_t arca_tuple_set(arcad tuple, size_t index, arcad value) {
  return syscall3(__NR_set, tuple, index, value);
}

int64_t arca_table_set(arcad table, size_t index,
                       const struct arca_entry *entry) {
  return syscall3(__NR_set, table, index, entry);
}

arcad arca_function_apply(arcad target, arcad argument) {
  return syscall2(__NR_apply, target, argument);
}

arcad arca_function_force(arcad target) { return syscall1(__NR_force, target); }

int64_t arca_table_map(arcad table, void *address, struct arca_entry *entry) {
  return syscall3(__NR_map, table, address, entry);
}

int64_t arca_mmap(void *address, struct arca_entry *entry) {
  return syscall2(__NR_mmap, address, entry);
}

int64_t arca_mprotect(void *address, int mode) {
  return syscall2(__NR_mprotect, address, mode);
}

int64_t arca_compat_mmap(void *address, size_t size, unsigned mode) {
  return syscall3(__NR_compat_mmap, address, size, mode);
}

arcad arca_call_with_current_continuation(arcad value) {
  return syscall1(__NR_call_with_current_continuation, value);
}

arcad arca_get_continuation(void) { return syscall0(__NR_get_continuation); }

int64_t arca_debug_log(const uint8_t *message, size_t len) {
  return syscall2(__NR_debug_log, message, len);
}

int64_t arca_debug_log_int(const uint8_t *message, size_t len, uint64_t value) {
  return syscall3(__NR_debug_log_int, message, len, value);
}

int64_t arca_debug_show(const uint8_t *message, size_t len, arcad value) {
  return syscall3(__NR_debug_show, message, len, value);
}
