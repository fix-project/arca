#ifndef _ARCA_SYS_H
#define _ARCA_SYS_H

#include "asm.h"
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef int64_t arcad;

struct arca_entry {
  unsigned mode;
  unsigned datatype;
  size_t data;
};

void arca_nop(void);
arcad arca_clone(arcad);
int64_t arca_drop(arcad);
[[noreturn]] void arca_exit(arcad);
int64_t arca_type(arcad);
arcad arca_argument(void);

arcad arca_null_create(void);
arcad arca_word_create(uint64_t value);
arcad arca_blob_create(const uint8_t *data, size_t len);
arcad arca_tuple_create(size_t len);
arcad arca_page_create(size_t size);
arcad arca_table_create(size_t size);
arcad arca_function_create(arcad data);

arcad arca_word_read(arcad word, uint64_t *output);
arcad arca_blob_read(arcad blob, size_t offset, uint8_t *data, size_t len);
arcad arca_page_read(arcad page, size_t offset, uint8_t *data, size_t len);
arcad arca_function_read(arcad function);

arcad arca_blob_write(arcad blob, size_t offset, const uint8_t *data,
                      size_t len);
arcad arca_page_write(arcad page, size_t offset, const uint8_t *data,
                      size_t len);

int64_t arca_equals(arcad x, arcad y);
int64_t arca_length(arcad value, size_t *output);
arcad arca_tuple_get(arcad value, size_t index);
arcad arca_table_get(arcad table, size_t index, struct arca_entry *entry);
arcad arca_tuple_set(arcad tuple, size_t index, arcad value);
arcad arca_table_set(arcad table, size_t index, const struct arca_entry *entry);

arcad arca_function_apply(arcad function, arcad argument);
arcad arca_function_force(arcad function);

int64_t arca_table_map(arcad table, void *address, struct arca_entry *entry);

int64_t arca_mmap(void *address, struct arca_entry *entry);
int64_t arca_mprotect(void *address, int mode);
int64_t arca_compat_mmap(void *address, size_t size, unsigned mode);

int64_t arca_call_with_current_continuation(arcad value);
arcad arca_get_continuation(void);

int64_t arca_debug_log(const uint8_t *message, size_t len);
int64_t arca_debug_log_int(const uint8_t *message, size_t len, uint64_t value);
int64_t arca_debug_show(const uint8_t *message, size_t len, arcad value);

#endif
