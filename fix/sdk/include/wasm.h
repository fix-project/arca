#ifndef FIX_WASM_H
#define FIX_WASM_H

#include <stdint.h>

#define wasm_table_get(table, index) __builtin_wasm_table_get(table, index)
#define wasm_table_set(table, index, value)                                    \
    __builtin_wasm_table_set(table, index, value)
#define wasm_table_size(table) __builtin_wasm_table_size(table)
#define wasm_table_grow(table, value, count)                                   \
    __builtin_wasm_table_grow(table, value, count)
#define wasm_table_fill(table, index, value, count)                            \
    __builtin_wasm_table_fill(table, index, value, count)
#define wasm_table_copy(destination, source, source_index, destination_index,  \
                        count)                                                 \
    __builtin_wasm_table_copy(destination, source, source_index,               \
                              destination_index, count)

#define WASM_POSTPROCESSOR_IMPORT(name)                                        \
    __attribute__((import_module("__wasm_postprocessor"), import_name(name)))

#define WASM_MEMORY(index) WASM_MEMORY_DECLARE(index)
#define WASM_MEMORY_DECLARE(index)                                             \
    WASM_POSTPROCESSOR_IMPORT("memory_size_" #index)                           \
    uint32_t wasm_memory_size_##index(void);                                   \
    WASM_POSTPROCESSOR_IMPORT("memory_grow_" #index)                           \
    int32_t wasm_memory_grow_##index(uint32_t pages);                          \
    WASM_POSTPROCESSOR_IMPORT("memory_fill_" #index)                           \
    void wasm_memory_fill_##index(void *destination, uint32_t byte,            \
                                  uint32_t length)

#define WASM_MEMORY_COPY(source, destination)                                  \
    WASM_MEMORY_COPY_DECLARE(source, destination)
#define WASM_MEMORY_COPY_DECLARE(source, destination)                          \
    WASM_POSTPROCESSOR_IMPORT("memory_copy_" #source "_to_" #destination)      \
    void wasm_memory_copy_##source##_to_##destination(                         \
        void *destination_offset, const void *source_offset, uint32_t length)

#define WASM_TABLE(index) WASM_TABLE_DECLARE(index)
#define WASM_TABLE_DECLARE(index)                                              \
    WASM_POSTPROCESSOR_IMPORT("table_size_" #index)                            \
    uint32_t wasm_table_size_##index(void);                                    \
    WASM_POSTPROCESSOR_IMPORT("table_grow_" #index)                            \
    int32_t wasm_table_grow_##index(__externref_t value, uint32_t count);      \
    WASM_POSTPROCESSOR_IMPORT("table_get_" #index)                             \
    __externref_t wasm_table_get_##index(uint32_t offset);                     \
    WASM_POSTPROCESSOR_IMPORT("table_set_" #index)                             \
    void wasm_table_set_##index(uint32_t offset, __externref_t value);         \
    WASM_POSTPROCESSOR_IMPORT("table_fill_" #index)                            \
    void wasm_table_fill_##index(uint32_t offset, __externref_t value,         \
                                 uint32_t count)

#define WASM_TABLE_COPY(source, destination)                                   \
    WASM_TABLE_COPY_DECLARE(source, destination)
#define WASM_TABLE_COPY_DECLARE(source, destination)                           \
    WASM_POSTPROCESSOR_IMPORT("table_copy_" #source "_to_" #destination)       \
    void wasm_table_copy_##source##_to_##destination(                          \
        uint32_t destination_offset, uint32_t source_offset, uint32_t count)

#endif
