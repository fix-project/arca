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
#include "wasm-rt.h"

#include <assert.h>
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <sys/mman.h>

[[noreturn]] void trap(const char *msg);
uint64_t check(const char *msg, int64_t ret);

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

static void *os_mmap(size_t size) {
  int map_prot = PROT_NONE;
  int map_flags = MAP_ANONYMOUS | MAP_PRIVATE;
  uint8_t *addr = (uint8_t *)(check(
      "mmap", (int64_t)mmap(NULL, size, map_prot, map_flags, -1, 0)));
  return addr;
}

static int os_mprotect(void *addr, size_t size) {
  return mprotect(addr, size, PROT_READ | PROT_WRITE);
}

static int os_munmap(void *addr, size_t size) {
  return check("munmap", munmap(addr, size));
}

void wasm_rt_allocate_memory(wasm_rt_memory_t *memory, uint64_t initial_pages,
                             uint64_t max_pages, bool is64) {
  assert(max_pages <= (1ul << 32) / PAGE_SIZE);

  uint64_t byte_length = initial_pages * PAGE_SIZE;
  void *addr = os_mmap(1ul << 32);
  check("os_mprotect", os_mprotect(addr, byte_length));

  memory->data = (uint8_t *)(addr);
  memory->size = byte_length;
  memory->pages = initial_pages;
  memory->max_pages = max_pages;
  memory->is64 = is64;
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

  int ret = os_mprotect(memory->data + old_size, delta_size);

  if (ret != 0) {
    return -1;
  }

  memory->pages = new_pages;
  memory->size = new_size;
  return old_pages;
}

void wasm_rt_free_memory(wasm_rt_memory_t *memory) {
  os_munmap(memory->data, memory->size);
}

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
DEFINE_TABLE_OPS(externref)

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
