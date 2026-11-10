#ifndef FIX_SHELL_ABI_H
#define FIX_SHELL_ABI_H
#include "module.h"
#include <stdint.h>
struct w2c_fix;
void w2c_fix_advise(struct w2c_fix *, uint32_t, uint32_t, uint32_t, uint32_t,
                    uint32_t);
uint32_t w2c_fix_coordinate(struct w2c_fix *, uint32_t, uint32_t, uint32_t,
                            uint32_t);
uint32_t w2c_fix_get_type(struct w2c_fix *, uint32_t);
void w2c_fix_close(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_blob(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_attach_blob(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_access_blob(struct w2c_fix *, uint32_t, uint32_t, uint32_t,
                             uint32_t);
void w2c_fix_detach_blob(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_create_tree(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_access_tree(struct w2c_fix *, uint32_t, uint32_t, uint32_t,
                             uint32_t);
uint32_t w2c_fix_create_tag(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_get_data_type(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_ref(struct w2c_fix *, uint32_t);
uint64_t w2c_fix_len(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_is_comparable(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_equals(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_create_application_thunk(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_identification_thunk(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_selection_thunk(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_strict_encode(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_shallow_encode(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_get_encode_type(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_call_cc(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_return_cc(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_eval(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_clone(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_is_value(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_is_tag(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_get_thunk_type(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_unsound_handle_bytes(struct w2c_fix *, uint32_t);
void w2c_fix_read_handle_bytes(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_same_handle_bytes(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_create_tag_unchecked(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_blob_i32(struct w2c_fix *, uint32_t);
uint32_t w2c_fix_create_blob_i64(struct w2c_fix *, uint64_t);
uint32_t w2c_fix_get_tree_element(struct w2c_fix *, uint32_t, uint32_t);
void w2c_fix_attach_tree_bytes(struct w2c_fix *, uint32_t, uint32_t);
uint32_t w2c_fix_effect(struct w2c_fix *, uint32_t);
uint32_t w2c_module_0x5Ffix_apply(w2c_module *, uint32_t);
#endif
