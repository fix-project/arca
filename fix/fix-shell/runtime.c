#include "runtime.h"
#include "fix.h"

#include <arca/arca.h>
#include <arca/sys.h>
#include <assert.h>

extern wasm_rt_memory_t *WASM_MEMORIES[128];
extern size_t WASM_MEMORIES_N;

static size_t bytes_to_wasm_pages(size_t bytes) {
  return (bytes + PAGE_SIZE - 1) / PAGE_SIZE;
}

// Create an Arca page table of length `wasm_pages` of Wasm pages
static arcad create_wasm_pages(size_t wasm_pages, size_t page_size) {
  size_t bytes = wasm_pages * PAGE_SIZE;
  size_t pages = (bytes + page_size) / page_size;
  // Creates a page table that covers the length of bytes
  arcad table = arca_table_create(bytes);
  // For every page in the table, create a rw 4k page
  for (size_t i = 0; i < pages; i++) {
    struct arca_entry entry;
    entry.mode = __MODE_read_write;
    // Create a rw 4k page
    entry.data = check("arca_page_create", arca_page_create(page_size));
    // Map the page as rw in the table
    arca_table_map(table, (void *)(i * page_size), &entry);
  }
  return table;
}

static struct arca_entry map_table(void *addr, arcad table, bool write) {
  struct arca_entry entry;
  entry.mode = write ? __MODE_read_write : __MODE_read_only;
  entry.data = table;
  check("arca_mmap", arca_mmap(addr, &entry));
  return entry;
}

static void check_cond(bool predicate) {
  if (!predicate) {
    arca_panic("Assertion failed");
  }
}

wasm_rt_externref_t w2c_fixpoint_get_tree_entry(struct w2c_fixpoint *instance,
                                                wasm_rt_externref_t handle,
                                                uint32_t index) {
  // Check whether `handle` refers to a TreeObject
  if (handle.type != TreeObject) {
    arca_log("get_tree_entry: handle does not refer to a TreeObject");
    arca_panic("get_tree_entry: handle does not refer to a TreeObject");
  }

  arcad type = check("arca_tuple_get", arca_tuple_get(handle.d, index * 2));
  arcad data = check("arca_tuple_get", arca_tuple_get(handle.d, index * 2 + 1));

  return arcad_to_handle(type, data);
}

wasm_rt_externref_t w2c_fixpoint_create_blob_i64(struct w2c_fixpoint *instance,
                                                 uint64_t val) {
  arcad data = check("arca_word_create", arca_word_create(val));
  arcad type = type_to_arcad(BlobObject);
  return arcad_to_handle(type, data);
}

void w2c_fixpoint_attach_blob(struct w2c_fixpoint *instance, uint32_t n,
                              wasm_rt_externref_t handle) {
  if (handle.type != BlobObject) {
    arca_log("attach_blob: handle does not refer to a BlobObject");
    arca_panic("attach_blob: handle does not refer to a BlobObject");
  }

  arcad d = handle.d;

  // check_cond(n < WASM_MEMORIES_N);
  wasm_rt_memory_t *memory = WASM_MEMORIES[n];
  // `addr` is the beginning address of this wasm memory in the address space
  void *addr = (void *)((size_t)n << 32);

  // Setup size fields of the wasm memory
  size_t nbytes;
  check("arca_length", arca_length(d, &nbytes));
  size_t npages = bytes_to_wasm_pages(nbytes);
  memory->size = nbytes;
  memory->pages = npages;

  arcad pages;
  struct arca_entry entry;
  switch (arca_type(handle.d)) {
  case __TYPE_word: {
    check_cond(npages == 1);
    // Create rw page table and map at `addr`
    pages = create_wasm_pages(npages, 4096);
    entry = map_table(addr, pages, true);
    check_cond(entry.mode == __MODE_none);
    // Read the blob content to `addr`
    arca_word_read(handle.d, addr);

    // arca_mmap returns the old entry in the page table at `addr`
    arca_mmap(addr, &entry);
    check_cond(entry.mode == __MODE_read_write);
    // set the entry mode to read only
    entry.mode = __MODE_read_only;
    // map the entry back at the same location; now `addr` is read only and
    // contains the word
    arca_mmap(addr, &entry);
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  case __TYPE_blob: {
    pages = create_wasm_pages(npages, 4096);
    entry = map_table(addr, pages, true);
    arca_blob_read(handle.d, 0, addr, nbytes);
    arca_mmap(addr, &entry);
    entry.mode = __MODE_read_only;
    arca_mmap(addr, &entry);
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  case __TYPE_page: {
    // Check the size of the page
    size_t page_size;
    check("arca_length", arca_length(d, &page_size));
    pages = create_wasm_pages(npages, page_size);

    // Map the page for Blob to the created table
    entry.mode = __MODE_read_only;
    entry.data = d;
    arca_table_set(pages, 0, &entry);

    // Map the table at `addr`
    entry = map_table(addr, pages, false);

    // Drop the old entry at `addr` if any
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  case __TYPE_table: {
    entry = map_table(addr, handle.d, false);

    // Drop the old entry at `addr` if any
    if (entry.mode != __MODE_none) {
      arca_drop(entry.data);
    }
    return;
  }

  default:
    check_cond(false);
  }

  return;
}
