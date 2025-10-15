#pragma once

#include "wasm-rt.h"
#include <stdint.h>

// Get `index`th entry from a Tree
wasm_rt_externref_t w2c_fixpoint_get_tree_entry(struct w2c_fixpoint *instance,
                                                wasm_rt_externref_t handle,
                                                uint32_t index);
// Attach the Blob referred by `handle` to `index`th wasm memory
void w2c_fixpoint_attach_blob(struct w2c_fixpoint *instance, uint32_t index,
                              wasm_rt_externref_t handle);
// Create a Blob with content `val`
wasm_rt_externref_t w2c_fixpoint_create_blob_i64(struct w2c_fixpoint *instance,
                                                 uint64_t val);
