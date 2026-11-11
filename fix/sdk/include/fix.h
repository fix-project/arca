#ifndef FIX_SDK_FIX_H
#define FIX_SDK_FIX_H
#include <stdint.h>
typedef uint32_t fix_handle;
__attribute__((import_module("fix"), import_name("exit"), noreturn)) void
fix_exit(fix_handle value);
typedef enum { FIX_OBJECT, FIX_REF, FIX_THUNK, FIX_ENCODE } fix_value_type;
typedef enum { FIX_BLOB, FIX_TREE } fix_data_type;
typedef enum { FIX_STRICT, FIX_SHALLOW } fix_encode_type;
typedef enum {
    FIX_APPLICATION,
    FIX_SELECTION,
    FIX_IDENTIFICATION
} fix_thunk_type;
__attribute__((import_module("fix"), import_name("get_type"))) uint32_t
fix_get_type(uint32_t arg0);
__attribute__((import_module("fix"), import_name("close"))) void
fix_close(uint32_t arg0);
__attribute__((import_module("fix"), import_name("create_blob"))) uint32_t
fix_create_blob(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"), import_name("attach_blob"))) uint32_t
fix_attach_blob(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"), import_name("access_blob"))) uint32_t
fix_access_blob(fix_handle blob, uint32_t offset, void *destination,
                uint32_t length);
__attribute__((import_module("fix"), import_name("detach_blob"))) void
fix_detach_blob(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"), import_name("create_tree"))) uint32_t
fix_create_tree(const fix_handle *values, uint32_t length);
__attribute__((import_module("fix"), import_name("access_tree"))) uint32_t
fix_access_tree(fix_handle tree, uint32_t offset, fix_handle *destination,
                uint32_t length);
__attribute__((import_module("fix"), import_name("create_tag"))) uint32_t
fix_create_tag(uint32_t arg0);
__attribute__((import_module("fix"), import_name("get_data_type"))) uint32_t
fix_get_data_type(uint32_t arg0);
__attribute__((import_module("fix"), import_name("create_ref"))) uint32_t
fix_create_ref(uint32_t arg0);
__attribute__((import_module("fix"), import_name("len"))) uint64_t
fix_len(uint32_t arg0);
__attribute__((import_module("fix"), import_name("is_comparable"))) uint32_t
fix_is_comparable(uint32_t arg0);
__attribute__((import_module("fix"), import_name("equals"))) uint32_t
fix_equals(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"),
               import_name("create_application_thunk"))) uint32_t
fix_create_application_thunk(uint32_t arg0);
__attribute__((import_module("fix"),
               import_name("create_identification_thunk"))) uint32_t
fix_create_identification_thunk(uint32_t arg0);
__attribute__((import_module("fix"),
               import_name("create_selection_thunk"))) uint32_t
fix_create_selection_thunk(uint32_t arg0);
__attribute__((import_module("fix"),
               import_name("create_strict_encode"))) uint32_t
fix_create_strict_encode(uint32_t arg0);
__attribute__((import_module("fix"),
               import_name("create_shallow_encode"))) uint32_t
fix_create_shallow_encode(uint32_t arg0);
__attribute__((import_module("fix"), import_name("get_encode_type"))) uint32_t
fix_get_encode_type(uint32_t arg0);
__attribute__((import_module("fix"), import_name("call_cc"))) uint32_t
fix_call_cc(uint32_t arg0);
__attribute__((import_module("fix"), import_name("return_cc"))) uint32_t
fix_return_cc(uint32_t arg0);
__attribute__((import_module("fix"), import_name("eval"))) uint32_t
fix_eval(uint32_t arg0);
__attribute__((import_module("fix"), import_name("clone"))) uint32_t
fix_clone(uint32_t arg0);
__attribute__((import_module("fix"), import_name("is_value"))) uint32_t
fix_is_value(uint32_t arg0);
__attribute__((import_module("fix"), import_name("is_tag"))) uint32_t
fix_is_tag(uint32_t arg0);
__attribute__((import_module("fix"), import_name("get_thunk_type"))) uint32_t
fix_get_thunk_type(uint32_t arg0);
__attribute__((import_module("fix"),
               import_name("unsound_handle_bytes"))) uint32_t
fix_unsound_handle_bytes(uint32_t arg0);
__attribute__((import_module("fix"), import_name("read_handle_bytes"))) void
fix_read_handle_bytes(fix_handle value, void *destination);
__attribute__((import_module("fix"), import_name("same_handle_bytes"))) uint32_t
fix_same_handle_bytes(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"),
               import_name("create_tag_unchecked"))) uint32_t
fix_create_tag_unchecked(uint32_t arg0);
__attribute__((import_module("fix"), import_name("create_blob_i32"))) uint32_t
fix_create_blob_i32(uint32_t arg0);
__attribute__((import_module("fix"), import_name("create_blob_i64"))) uint32_t
fix_create_blob_i64(uint64_t arg0);
__attribute__((import_module("fix"), import_name("get_tree_element"))) uint32_t
fix_get_tree_element(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"), import_name("attach_tree_bytes"))) void
fix_attach_tree_bytes(uint32_t arg0, uint32_t arg1);
__attribute__((import_module("fix"), import_name("effect"))) uint32_t
fix_effect(uint32_t arg0);
__attribute__((import_module("fix"), import_name("coordinate"))) fix_handle
fix_coordinate(const uint8_t *name, uint32_t name_length, fix_handle arguments,
               uint64_t *status);
__attribute__((import_module("fix"), import_name("advise"))) void
fix_advise(fix_handle value, const char *key, uint32_t key_length,
           const void *payload, uint32_t payload_length);
static inline int32_t fix_is_object(fix_handle value) {
    return fix_get_type(value) == FIX_OBJECT;
}
static inline int32_t fix_is_data(fix_handle value) {
    return fix_get_type(value) <= FIX_REF;
}
static inline int32_t fix_is_blob_obj(fix_handle value) {
    return fix_is_object(value) && fix_get_data_type(value) == FIX_BLOB;
}
#endif
