/*
 * Copyright 2018 WebAssembly Community Group participants
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include "wasm-rt-impl.h"
#include <arca/sys.h>
#include <arca/arca.h>
#include "wasm-rt.h"

#include <assert.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>

wasm_rt_memory_t *WASM_MEMORIES[128];
size_t WASM_MEMORIES_N = 0;
wasm_rt_externref_table_t *WASM_TABLES[128];
size_t WASM_TABLES_N = 0;

long check(char *msg, long ret);
[[noreturn]] void trap(const char *msg);

void wasm_rt_trap(wasm_rt_trap_t code) {
  assert(code != WASM_RT_TRAP_NONE);
  switch (code) {
  case WASM_RT_TRAP_NONE:
    trap("Wasm Runtime Trap: None");
  case WASM_RT_TRAP_OOB:
    trap(
        "Wasm Runtime Trap: Out-of-bounds access in linear memory or a table.");
  case WASM_RT_TRAP_INT_OVERFLOW:
    trap("Wasm Runtime Trap: Integer overflow on divide or truncation.");
  case WASM_RT_TRAP_DIV_BY_ZERO:
    trap("Wasm Runtime Trap: Integer divide by zero");
  case WASM_RT_TRAP_INVALID_CONVERSION:
    trap("Wasm Runtime Trap: Conversion from NaN to integer.");
  case WASM_RT_TRAP_UNREACHABLE:
    trap("Wasm Runtime Trap: Unreachable instruction executed.");
  case WASM_RT_TRAP_CALL_INDIRECT: /** Invalid call_indirect, for any reason.
                                    */
    trap("Wasm Runtime Trap: Invalid call_indirect.");
  case WASM_RT_TRAP_UNCAUGHT_EXCEPTION:
    trap("Wasm Runtime Trap: Exception thrown and not caught.");
  case WASM_RT_TRAP_UNALIGNED:
    trap("Wasm Runtime Trap: Unaligned atomic instruction executed.");
#if WASM_RT_MERGED_OOB_AND_EXHAUSTION_TRAPS
  case WASM_RT_TRAP_EXHAUSTION = WASM_RT_TRAP_OOB:
#else
  case WASM_RT_TRAP_EXHAUSTION:
    trap("Wasm Runtime Trap: Call stack exhausted.");
#endif
  };
  abort();
}

void wasm_rt_init(void) {}

bool wasm_rt_is_initialized(void) { return true; }

void wasm_rt_free(void) {}

void wasm_rt_allocate_memory(wasm_rt_memory_t *memory, uint64_t initial_pages,
                             uint64_t max_pages, bool is64) {
  size_t n = WASM_MEMORIES_N++;

  assert(n < 128);
  WASM_MEMORIES[n] = memory;
  assert(max_pages <= ((1ul << 32) / PAGE_SIZE));

  memory->data = (void *)(n << 32);
  uint64_t byte_length = initial_pages * PAGE_SIZE;
  memory->size = byte_length;
  memory->pages = initial_pages;
  memory->max_pages = max_pages;
  memory->is64 = is64;

  for (uint64_t i = 0; i < byte_length >> 12; i++) {
    arcad page = check("arca_page_create", arca_page_create(1 << 12));
    check("arca_mmap",
          arca_mmap(memory->data + i * 4096, &(struct arca_entry){
                                                 .mode = __MODE_read_write,
                                                 .data = page,
                                             }));
  }
  return;
}

uint64_t wasm_rt_grow_memory(wasm_rt_memory_t *memory, uint64_t delta) {
  uint64_t old_pages = memory->pages;
  uint64_t new_pages = memory->pages + delta;
  if (new_pages == 0) {
    return 0;
  }
  if (new_pages < old_pages || new_pages > memory->max_pages) {
    return (uint64_t)-1;
  }
  uint64_t old_size = old_pages * PAGE_SIZE;
  uint64_t new_size = new_pages * PAGE_SIZE;
  uint64_t delta_size = delta * PAGE_SIZE;

  for (uint64_t i = 0; i < delta_size >> 12; i++) {
    arcad page = check("arca_page_create", arca_page_create(1 << 12));
    check("arca_mmap", arca_mmap(memory->data + +memory->size + i * 4096,
                                 &(struct arca_entry){
                                     .mode = __MODE_read_write,
                                     .data = page,
                                 }));
  }

  memory->pages = new_pages;
  memory->size = new_size;
  return old_pages;
}

void wasm_rt_free_memory(wasm_rt_memory_t *memory) { return; }

#define DEFINE_TABLE_OPS(type)                                                 \
  void wasm_rt_allocate_##type##_table(wasm_rt_##type##_table_t *table,        \
                                       uint32_t elements,                      \
                                       uint32_t max_elements) {                \
    abort();                                                                   \
  }                                                                            \
  void wasm_rt_free_##type##_table(wasm_rt_##type##_table_t *table) {          \
    abort();                                                                   \
  }                                                                            \
  uint32_t wasm_rt_grow_##type##_table(wasm_rt_##type##_table_t *table,        \
                                       uint32_t delta,                         \
                                       wasm_rt_##type##_t init) {              \
    abort();                                                                   \
  }

DEFINE_TABLE_OPS(funcref)

void wasm_rt_allocate_externref_table(wasm_rt_externref_table_t *table,
                                      uint32_t elements,
                                      uint32_t max_elements) {
  size_t n = WASM_TABLES_N++;
  assert(n < 128);
  WASM_TABLES[n] = table;

  if (max_elements > ((1ull << 32) / sizeof(wasm_rt_externref_t)) ) {
    max_elements = (1ull << 32) / sizeof(wasm_rt_externref_t);
  }
  assert(max_elements * sizeof(wasm_rt_externref_t) <= (1ull << 32));

  // tables are after the memories in the address space
  table->data = (void *)((128 + n) << 32);
  table->max_size = max_elements;
  table->size = elements;

  uint64_t byte_length = elements * sizeof(wasm_rt_externref_t);
  uint64_t num_pages = (byte_length + (1ull << 12) - 1) / (1ull << 12);

  for (uint64_t i = 0; i < num_pages; i++) {
    arcad page = check("arca_page_create", arca_page_create(1 << 12));
    check("arca_mmap", arca_mmap((uint8_t *)(table->data) + i * 4096,
                                 &(struct arca_entry){
                                     .mode = __MODE_read_write,
                                     .data = page,
                                 }));
  }
  return;
}

void wasm_rt_free_externref_table(wasm_rt_externref_table_t *table) { return; }

uint32_t wasm_rt_grow_externref_table(wasm_rt_externref_table_t *table,
                                      uint32_t delta,
                                      wasm_rt_externref_t init) {
  uint64_t old_elements = table->size;
  uint64_t new_elements = old_elements + delta;
  if (new_elements == 0) {
    return 0;
  }
  if (new_elements < old_elements || new_elements > table->max_size) {
    return (uint32_t)-1;
  }
  uint64_t old_size = old_elements * sizeof(wasm_rt_externref_t);
  uint64_t new_size = new_elements * sizeof(wasm_rt_externref_t);

  uint64_t old_num_pages = (old_size + (1ull << 12) - 1) / (1ull << 12);
  uint64_t new_num_pages = (new_size + (1ull << 12) - 1) / (1ull << 12);

  for (uint64_t i = 0; i < new_num_pages - old_num_pages; i++) {
    arcad page = check("arca_page_create", arca_page_create(1 << 12));
    check("arca_mmap",
          arca_mmap((uint8_t *)(table->data) + old_num_pages * 4096 + i * 4096,
                    &(struct arca_entry){
                        .mode = __MODE_read_write,
                        .data = page,
                    }));
  }

  table->size = new_elements;
  return old_elements;
}

const char *wasm_rt_strerror(wasm_rt_trap_t trap) {
  switch (trap) {
  case WASM_RT_TRAP_NONE:
    return "No error";
  case WASM_RT_TRAP_OOB:
#if WASM_RT_MERGED_OOB_AND_EXHAUSTION_TRAPS
    return "Out-of-bounds access in linear memory or a table, or call stack "
           "exhausted";
#else
    return "Out-of-bounds access in linear memory or a table";
  case WASM_RT_TRAP_EXHAUSTION:
    return "Call stack exhausted";
#endif
  case WASM_RT_TRAP_INT_OVERFLOW:
    return "Integer overflow on divide or truncation";
  case WASM_RT_TRAP_DIV_BY_ZERO:
    return "Integer divide by zero";
  case WASM_RT_TRAP_INVALID_CONVERSION:
    return "Conversion from NaN to integer";
  case WASM_RT_TRAP_UNREACHABLE:
    return "Unreachable instruction executed";
  case WASM_RT_TRAP_CALL_INDIRECT:
    return "Invalid call_indirect or return_call_indirect";
  case WASM_RT_TRAP_UNCAUGHT_EXCEPTION:
    return "Uncaught exception";
  case WASM_RT_TRAP_UNALIGNED:
    return "Unaligned atomic memory access";
  }
  return "invalid trap code";
}
