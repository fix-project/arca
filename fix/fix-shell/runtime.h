#pragma once

#include "wasm-rt.h"
#include <stdint.h>

typedef struct w2c_fixpoint w2c_fixpoint;

// Attach the Blob referred by `handle` to `index`th wasm memory
void w2c_fixpoint_attach_blob(struct w2c_fixpoint *instance, uint32_t index,
                              wasm_rt_externref_t handle);
// Attach the Tree referred by `handle` to `index`th wasm table 
void w2c_fixpoint_attach_tree(struct w2c_fixpoint *instance, uint32_t index,
                              wasm_rt_externref_t handle);
// Create a Blob with content `val`
wasm_rt_externref_t w2c_fixpoint_create_blob_i64(struct w2c_fixpoint *instance,
                                                 uint64_t val);
