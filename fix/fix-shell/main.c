#include "module.h"
#include "wasm-rt.h"


#include <arca/sys.h>
#include <assert.h>
#include <stdbool.h>

#define SELF_PAGE_TABLE 0

extern wasm_rt_memory_t *WASM_MEMORIES[128];
extern size_t WASM_MEMORIES_N;

static int len(const char *s) {
  int i = 0;
  while (s[i])
    i++;
  return i;
}

static void error_append(const char *msg) {
  arca_debug_log((const uint8_t *)msg, len(msg));
}

static void error_append_int(const char *msg, int value) {
  arca_debug_log_int((const uint8_t* )msg, len(msg), value);
}

[[noreturn]] void trap(const char *msg) {
  error_append(msg);
  arca_exit(0);
}

[[noreturn]] void abort(void) {
  error_append("abort");
  arca_exit(0);
}

uint64_t check(int64_t ret) {
  assert(ret >= 0);
  return ret;
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i64(struct w2c_fixpoint *instance,
                                                 uint64_t val) {
  return check(arca_word_create(val));
}

wasm_rt_externref_t w2c_fixpoint_get_tree_entry(struct w2c_fixpoint *instance,
                                                wasm_rt_externref_t handle,
                                                uint32_t index) {
  arcad entry = check(arca_tuple_get(handle, index));
  return entry;
}

static size_t bytes_to_wasm_pages(size_t bytes) {
  return (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
}

static arcad create_wasm_pages(size_t wasm_pages) {
  size_t bytes = wasm_pages * PAGE_SIZE;
  size_t pages = (bytes + 4095) / 4096;
  arcad table = arca_table_create(bytes);
  for (size_t i = 0; i < pages; i++) {
    struct arca_entry entry;
    entry.mode = __MODE_read_write;
    entry.data = check(arca_page_create(4096));
    arca_table_map(table, (void *)(i * 4096), &entry);
  }
  return table;
}

static struct arca_entry map_table(void *addr, arcad table, bool write) {
  struct arca_entry entry;
  entry.mode = write ? __MODE_read_write: __MODE_read_only;
  entry.data = table;
  check(arca_mmap(addr, &entry));
  return entry;
}

void w2c_fixpoint_attach_blob(struct w2c_fixpoint *instance, uint32_t n,
                              wasm_rt_externref_t handle) {
  assert(n < WASM_MEMORIES_N);
  wasm_rt_memory_t *memory = WASM_MEMORIES[n];
  void *addr = (void *)((size_t)n << 32);

  size_t nbytes;
  check(arca_length(handle, &nbytes));
  size_t npages = bytes_to_wasm_pages(nbytes);
  memory->size = nbytes;
  memory->pages = npages;

  // TODO: map these blobs as read-only
  arcad pages;
  struct arca_entry entry;
  switch (arca_type(handle)) {
  case __TYPE_word: {
    assert(npages == 1);
    pages = create_wasm_pages(npages);
    entry = map_table(addr, pages, true);
    assert(entry.mode == __MODE_none);
    arca_word_read(handle, addr);
    arca_mmap(addr, &entry);
    assert(entry.mode == __MODE_read_write);
    entry.mode = __MODE_read_only;
    arca_mmap(addr, &entry);
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  case __TYPE_blob: {
    pages = check(create_wasm_pages(npages));
    entry = map_table(addr, pages, true);
    arca_blob_read(handle, 0, addr, nbytes);
    arca_mmap(addr, &entry);
    entry.mode = __MODE_read_only;
    arca_mmap(addr, &entry);
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  case __TYPE_page: {
    pages = check(create_wasm_pages(npages));
    entry = map_table(addr, pages, true);
    arca_page_read(handle, 0, addr, nbytes);
    arca_mmap(addr, &entry);
    entry.mode = __MODE_read_only;
    arca_mmap(addr, &entry);
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  case __TYPE_table: {
    map_table(addr, handle, false);
    return;
  }

  default:
    assert(false);
  }

  return;
}

[[noreturn]] void fmain(void) {
  w2c_module module;
  wasm2c_module_instantiate(&module, (struct w2c_fixpoint *)&module);
  wasm_rt_externref_t argument = arca_argument();
  wasm_rt_externref_t result = w2c_module_0x5Ffixpoint_apply(&module, argument);
  arca_exit(result);
}
