#include "syscall.h"

extern arcad syscall(enum arca_syscall num, ...);

[[noreturn]] static void ud2() {
  for (;;) {
    asm("ud2");
  }
}

void arca_nop(void) { syscall(SYS_NOP); }

arcad arca_clone(arcad value) { return syscall(SYS_CLONE, value); }

int64_t arca_drop(arcad value) { return syscall(SYS_DROP, value); }

[[noreturn]] void arca_exit(arcad value) {
  syscall(SYS_EXIT, value);
  ud2();
}

enum arca_datatype arca_type(arcad value) { return syscall(SYS_TYPE, value); }

arcad arca_null_create(void) { return syscall(SYS_CREATE_NULL); }

arcad arca_word_create(uint64_t value) {
  return syscall(SYS_CREATE_WORD, value);
}

arcad arca_atom_create(const uint8_t *data, size_t len) {
  return syscall(SYS_CREATE_ATOM, data, len);
}

arcad arca_error_create(arcad value) {
  return syscall(SYS_CREATE_ERROR, value);
}

arcad arca_blob_create(const uint8_t *data, size_t len) {
  return syscall(SYS_CREATE_BLOB, data, len);
}

arcad arca_tree_create(size_t len) { return syscall(SYS_CREATE_TREE, len); }

arcad arca_page_create(size_t size) { return syscall(SYS_CREATE_PAGE, size); }

arcad arca_table_create(size_t size) { return syscall(SYS_CREATE_TABLE, size); }

arcad arca_lambda_create(arcad thunk, size_t index) {
  return syscall(SYS_CREATE_LAMBDA, thunk, index);
}

arcad arca_thunk_create(arcad registers, arcad memory, arcad descriptors);

arcad arca_word_read(arcad word, uint64_t *output) {
  return syscall(SYS_READ, word, output);
}

arcad arca_error_read(arcad error) { return syscall(SYS_READ, error); }

arcad arca_blob_read(arcad blob, uint8_t *data, size_t len) {
  return syscall(SYS_READ, blob, data, len);
}

arcad arca_page_read(arcad page, size_t offset, uint8_t *data, size_t len) {
  return syscall(SYS_READ, page, offset, data, len);
}

arcad arca_page_write(arcad page, size_t offset, const uint8_t *data,
                      size_t len);

int64_t arca_equals(arcad x, arcad y) { return syscall(SYS_EQUALS, x, y); }

int64_t arca_length(arcad value, size_t *output) {
  return syscall(SYS_LENGTH, value, output);
}

arcad arca_tree_take(arcad value, size_t index) {
  return syscall(SYS_TAKE, value, index);
}

arcad arca_table_take(arcad table, size_t index, struct arca_entry *entry) {
  return syscall(SYS_TAKE, table, index, entry);
}

arcad arca_tree_put(arcad tree, size_t index, arcad value) {
  return syscall(SYS_PUT, tree, index, value);
}

arcad arca_table_put(arcad table, size_t index, struct arca_entry *entry) {
  return syscall(SYS_PUT, table, index, entry);
}

arcad arca_tree_get(arcad value, size_t index) {
  return syscall(SYS_GET, value, index);
}

arcad arca_table_get(arcad table, size_t index, struct arca_entry *entry) {
  return syscall(SYS_GET, table, index, entry);
}

int64_t arca_tree_set(arcad tree, size_t index, arcad value) {
  return syscall(SYS_SET, tree, index, value);
}

int64_t arca_table_set(arcad table, size_t index,
                       const struct arca_entry *entry) {
  return syscall(SYS_SET, table, index, entry);
}

arcad arca_apply(arcad target, arcad argument) {
  return syscall(SYS_APPLY, target, argument);
}

int64_t arca_table_map(arcad table, void *address, struct arca_entry *entry) {
  return syscall(SYS_MAP, table, address, entry);
}

int64_t arca_mmap(void *address, struct arca_entry *entry) {
  return syscall(SYS_MMAP, address, entry);
}

arcad arca_return_continuation_lambda(void) {
  return syscall(SYS_RETURN_CONTINUATION_LAMBDA);
}

arcad arca_call_with_current_continuation(arcad value) {
  return syscall(SYS_CALL_WITH_CURRENT_CONTINUATION, value);
}

arcad arca_capture_continuation_thunk(bool *continued) {
  return syscall(SYS_CAPTURE_CONTINUATION_THUNK, continued);
}

arcad arca_capture_continuation_lambda(bool *continued) {
  return syscall(SYS_CAPTURE_CONTINUATION_LAMBDA, continued);
}

int64_t arca_debug_log(const uint8_t *message, size_t len) {
  return syscall(SYS_DEBUG_LOG, message, len);
}

int64_t arca_debug_log_int(const uint8_t *message, size_t len, uint64_t value) {
  return syscall(SYS_DEBUG_LOG_INT, message, len, value);
}

int64_t arca_debug_show(const uint8_t *message, size_t len, arcad value) {
  return syscall(SYS_DEBUG_SHOW, message, len, value);
}

int64_t arca_error_reset(void) { return syscall(SYS_ERROR_RESET); }

int64_t arca_error_append(const uint8_t *message, size_t len) {
  return syscall(SYS_ERROR_APPEND, message, len);
}

int64_t arca_error_append_int(uint64_t val) {
  return syscall(SYS_ERROR_APPEND_INT, val);
}

[[noreturn]] void arca_error_return(void) {
  syscall(SYS_ERROR_RETURN);
  ud2();
}
