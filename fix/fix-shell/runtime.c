#include "bindings.h"
#include "runtime.h"

#include <arca/arca.h>
#include <arca/sys.h>
#include <assert.h>

extern wasm_rt_memory_t *WASM_MEMORIES[128];
extern size_t WASM_MEMORIES_N;
extern wasm_rt_externref_table_t *WASM_TABLES[128];
extern size_t WASM_TABLES_N;

static size_t bytes_to_wasm_pages(size_t bytes) {
  return (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i64(struct w2c_fixpoint *instance,
                                                 uint64_t val) {
  return (wasm_rt_externref_t)u8x32_from_bytes32(fixpoint_create_blob_i64(val));
}

void w2c_fixpoint_attach_blob(struct w2c_fixpoint *instance, uint32_t index,
                              wasm_rt_externref_t handle) {
  if (index >= WASM_MEMORIES_N) {
    arca_panic("memory index oob");
  }
  wasm_rt_memory_t *memory = WASM_MEMORIES[index];
  // `addr` is the beginning address of this wasm memory in the address space
  void *addr = (void *)(memory->data);
  uint64_t nbytes = fixpoint_attach_blob(addr, bytes32_from_u8x32(handle));

  size_t npages = bytes_to_wasm_pages(nbytes);
  memory->size = nbytes;
  memory->pages = npages;
  return;
}

void w2c_fixpoint_attach_tree(struct w2c_fixpoint *instance, uint32_t index,
                              wasm_rt_externref_t handle)
{
  if (index >= WASM_TABLES_N) {
    arca_panic("table index oob");
  }
  wasm_rt_externref_table_t *table = WASM_TABLES[index];
  // `addr` is the beginning address of this wasm memory in the address space
  void *addr = (void *)(table->data);
  uint64_t nelems = fixpoint_attach_tree(addr, bytes32_from_u8x32(handle));
  table->size = nelems;
  return;
}

long check(char *msg, long ret) {
  if (ret >= 0) {
    return ret;
  }
  arca_panic(msg);
}
