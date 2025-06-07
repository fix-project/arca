#pragma once
#include "defs.h"
#include "syscall.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

void arca_nop(void);
int64_t arca_clone(int64_t);
int64_t arca_drop(int64_t);
[[noreturn]] void arca_exit(int64_t);
enum arca_datatype arca_type(int64_t);

int64_t arca_null_create(void);
int64_t arca_word_create(uint64_t value);
int64_t arca_atom_create(const uint8_t *data, size_t len);
int64_t arca_error_create(int64_t value);
int64_t arca_blob_create(const uint8_t *data, size_t len);
int64_t arca_tree_create(size_t len);
int64_t arca_page_create(size_t size);
int64_t arca_table_create(size_t size);
int64_t arca_lambda_create(int64_t thunk, size_t index);
int64_t arca_thunk_create(int64_t registers, int64_t memory,
                          int64_t descriptors);

int64_t arca_word_read(int64_t word, uint64_t *output);
int64_t arca_error_read(int64_t error);
int64_t arca_blob_read(int64_t blob, uint8_t *data, size_t len);
int64_t arca_page_read(int64_t page, size_t offset, uint8_t *data, size_t len);

int64_t arca_page_write(int64_t page, size_t offset, const uint8_t *data,
                        size_t len);
int64_t arca_equals(int64_t x, int64_t y);
int64_t arca_length(int64_t value, size_t *output);
int64_t arca_tree_take(int64_t value, size_t index);
int64_t arca_table_take(int64_t table, size_t index, struct arca_entry *entry);
int64_t arca_tree_put(int64_t tree, size_t index, int64_t value);
int64_t arca_table_put(int64_t table, size_t index, struct arca_entry *entry);
int64_t arca_apply(int64_t lambda, int64_t argument);

int64_t arca_return_continuation_lambda(void);
int64_t arca_perform_effect(int64_t value);
[[noreturn]] void arca_tailcall(int64_t thunk);
int64_t arca_capture_continuation_thunk(bool *continued);
int64_t arca_capture_continuation_lambda(bool *continued);

int64_t arca_debug_log(const uint8_t *message, size_t len);
int64_t arca_debug_log_int(const uint8_t *message, size_t len, uint64_t value);
int64_t arca_debug_show(const uint8_t *message, size_t len, int64_t value);

int64_t arca_error_reset(void);
int64_t arca_error_append(const uint8_t *message, size_t len);
int64_t arca_error_append_int(uint64_t val);
[[noreturn]] int64_t arca_error_return(void);
