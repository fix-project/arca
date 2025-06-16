#pragma once
#include "defs.h"
#include "syscall.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

void arca_nop(void);
arcad arca_clone(arcad);
int64_t arca_drop(arcad);
[[noreturn]] void arca_exit(arcad);
enum arca_datatype arca_type(arcad);

arcad arca_null_create(void);
arcad arca_word_create(uint64_t value);
arcad arca_atom_create(const uint8_t *data, size_t len);
arcad arca_error_create(arcad value);
arcad arca_blob_create(const uint8_t *data, size_t len);
arcad arca_tree_create(size_t len);
arcad arca_page_create(size_t size);
arcad arca_table_create(size_t size);
arcad arca_lambda_create(arcad thunk, size_t index);
arcad arca_thunk_create(arcad registers, arcad memory, arcad descriptors);

arcad arca_word_read(arcad word, uint64_t *output);
arcad arca_error_read(arcad error);
arcad arca_blob_read(arcad blob, uint8_t *data, size_t len);
arcad arca_page_read(arcad page, size_t offset, uint8_t *data, size_t len);

arcad arca_page_write(arcad page, size_t offset, const uint8_t *data,
                      size_t len);
int64_t arca_equals(arcad x, arcad y);
int64_t arca_length(arcad value, size_t *output);
arcad arca_tree_take(arcad value, size_t index);
arcad arca_table_take(arcad table, size_t index, struct arca_entry *entry);
arcad arca_tree_put(arcad tree, size_t index, arcad value);
arcad arca_table_put(arcad table, size_t index, struct arca_entry *entry);
arcad arca_tree_get(arcad value, size_t index);
arcad arca_table_get(arcad table, size_t index, struct arca_entry *entry);
int64_t arca_tree_set(arcad tree, size_t index, arcad value);
int64_t arca_table_set(arcad table, size_t index,
                       const struct arca_entry *entry);
arcad arca_apply(arcad lambda, arcad argument);
int64_t arca_table_map(arcad table, void *address, struct arca_entry *entry);

int64_t arca_mmap(void *address, struct arca_entry *entry);
int64_t arca_mprotect(void *address, bool writeable);

arcad arca_return_continuation_lambda(void);
arcad arca_perform_effect(arcad value);
[[noreturn]] void arca_tailcall(arcad thunk);
arcad arca_capture_continuation_thunk(bool *continued);
arcad arca_capture_continuation_lambda(bool *continued);

int64_t arca_debug_log(const uint8_t *message, size_t len);
int64_t arca_debug_log_int(const uint8_t *message, size_t len, uint64_t value);
int64_t arca_debug_show(const uint8_t *message, size_t len, arcad value);

int64_t arca_error_reset(void);
int64_t arca_error_append(const uint8_t *message, size_t len);
int64_t arca_error_append_int(uint64_t val);
[[noreturn]] void arca_error_return(void);
