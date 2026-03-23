#include <assert.h>

#include "wasm-rt.h"

void wasm_rt_init(void) {
}

bool wasm_rt_is_initialized(void) {
  return true;
}

void wasm_rt_free(void) {
}

/**
 * Initialize a Memory object with an initial page size of `initial_pages` and
 * a maximum page size of `max_pages`, indexed with an i32 or i64.
 *
 *  ```
 *    wasm_rt_memory_t my_memory;
 *    // 1 initial page (65536 bytes), and a maximum of 2 pages,
 *    // indexed with an i32
 *    wasm_rt_allocate_memory(&my_memory, 1, 2, false);
 *  ```
 */
void wasm_rt_allocate_memory(wasm_rt_memory_t *, uint64_t initial_pages, uint64_t max_pages, bool is64) {
  assert(false);
}

/**
 * Initialize an externref Table object with an element count
 * of `elements` and a maximum size of `max_elements`.
 * Usage as per wasm_rt_allocate_funcref_table.
 */
void wasm_rt_allocate_externref_table(wasm_rt_externref_table_t *,
                                      uint32_t elements, uint32_t max_elements) {
  assert(false);
}

/**
 * Free a Memory object.
 */
void wasm_rt_free_memory(wasm_rt_memory_t *) {
  assert(false);
}

/**
 * Free an externref Table object.
 */
void wasm_rt_free_externref_table(wasm_rt_externref_table_t *) {
  assert(false);
}
