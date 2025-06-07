#include "syscall.h"

extern int64_t syscall(enum arca_syscall num, ...);

[[noreturn]] static void ud2() {
  for (;;) {
    asm("ud2");
  }
}

void arca_nop(void) { syscall(SYS_NOP); }

int64_t arca_clone(int64_t value) { return syscall(SYS_CLONE, value); }

int64_t arca_drop(int64_t value) { return syscall(SYS_DROP, value); }

[[noreturn]] void arca_exit(int64_t value) {
  syscall(SYS_EXIT, value);
  ud2();
}

enum arca_datatype arca_type(int64_t value) { return syscall(SYS_TYPE, value); }

int64_t arca_null_create(void) { return syscall(SYS_CREATE_NULL); }

int64_t arca_word_create(uint64_t value) {
  return syscall(SYS_CREATE_WORD, value);
}

int64_t arca_atom_create(const uint8_t *data, size_t len) {
  return syscall(SYS_CREATE_ATOM, data, len);
}

int64_t arca_error_create(int64_t value) {
  return syscall(SYS_CREATE_ERROR, value);
}

int64_t arca_blob_create(const uint8_t *data, size_t len) {
  return syscall(SYS_CREATE_BLOB, data, len);
}

int64_t arca_tree_create(size_t len) { return syscall(SYS_CREATE_TREE, len); }

int64_t arca_page_create(size_t size) { return syscall(SYS_CREATE_PAGE, size); }

int64_t arca_table_create(size_t size) {
  return syscall(SYS_CREATE_TABLE, size);
}

int64_t arca_lambda_create(int64_t thunk, size_t index) {
  return syscall(SYS_CREATE_LAMBDA, thunk, index);
}

int64_t arca_thunk_create(int64_t registers, int64_t memory,
                          int64_t descriptors);

int64_t arca_word_read(int64_t word, uint64_t *output) {
  return syscall(SYS_READ, word, output);
}

int64_t arca_error_read(int64_t error) { return syscall(SYS_READ, error); }

int64_t arca_blob_read(int64_t blob, uint8_t *data, size_t len) {
  return syscall(SYS_READ, blob, data, len);
}

int64_t arca_page_read(int64_t page, size_t offset, uint8_t *data, size_t len) {
  return syscall(SYS_READ, page, offset, data, len);
}

int64_t arca_page_write(int64_t page, size_t offset, const uint8_t *data,
                        size_t len);

int64_t arca_equals(int64_t x, int64_t y) { return syscall(SYS_EQUALS, x, y); }

int64_t arca_length(int64_t value, size_t *output) {
  return syscall(SYS_LENGTH, value, output);
}

int64_t arca_tree_take(int64_t value, size_t index) {
  return syscall(SYS_TAKE, value, index);
}

int64_t arca_table_take(int64_t table, size_t index, struct arca_entry *entry) {
  return syscall(SYS_TAKE, table, index, entry);
}

int64_t arca_tree_put(int64_t tree, size_t index, int64_t value) {
  return syscall(SYS_PUT, tree, index, value);
}

int64_t arca_table_put(int64_t table, size_t index, struct arca_entry *entry) {
  return syscall(SYS_PUT, table, index, entry);
}

int64_t arca_apply(int64_t lambda, int64_t argument) {
  return syscall(SYS_APPLY, lambda, argument);
}

int64_t arca_return_continuation_lambda(void) {
  return syscall(SYS_RETURN_CONTINUATION_LAMBDA);
}

int64_t arca_perform_effect(int64_t value) {
  return syscall(SYS_PERFORM_EFFECT, value);
}

[[noreturn]] void arca_tailcall(int64_t thunk) {
  syscall(SYS_TAILCALL, thunk);
  ud2();
}

int64_t arca_capture_continuation_thunk(bool *continued) {
  return syscall(SYS_CAPTURE_CONTINUATION_THUNK, continued);
}

int64_t arca_capture_continuation_lambda(bool *continued) {
  return syscall(SYS_CAPTURE_CONTINUATION_LAMBDA, continued);
}

int64_t arca_debug_log(const uint8_t *message, size_t len) {
  return syscall(SYS_DEBUG_LOG, message, len);
}

int64_t arca_debug_log_int(const uint8_t *message, size_t len, uint64_t value) {
  return syscall(SYS_DEBUG_LOG_INT, message, len, value);
}

int64_t arca_debug_show(const uint8_t *message, size_t len, int64_t value) {
  return syscall(SYS_DEBUG_SHOW, message, len, value);
}

int64_t arca_error_reset(void) { return syscall(SYS_ERROR_RESET); }

int64_t arca_error_append(const uint8_t *message, size_t len) {
  return syscall(SYS_ERROR_APPEND, message, len);
}

int64_t arca_error_append_int(uint64_t val) {
  return syscall(SYS_ERROR_APPEND_INT, val);
}

[[noreturn]] int64_t arca_error_return(void) {
  syscall(SYS_ERROR_RETURN);
  ud2();
}
