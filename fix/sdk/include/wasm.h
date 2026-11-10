#ifndef FIX_WASM_H
#define FIX_WASM_H

#include <stdint.h>

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

#endif
