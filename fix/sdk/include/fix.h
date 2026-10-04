#ifndef FIX_SDK_FIX_H
#define FIX_SDK_FIX_H

#include <stdint.h>

typedef __externref_t fix_handle;

typedef enum fix_value_type {
    FIX_OBJECT = 0,
    FIX_REF = 1,
    FIX_THUNK = 2,
    FIX_ENCODE = 3
} fix_value_type;
typedef enum fix_data_type { FIX_BLOB = 0, FIX_TREE = 1 } fix_data_type;
typedef enum fix_encode_type {
    FIX_STRICT = 0,
    FIX_SHALLOW = 1
} fix_encode_type;

#ifdef __cplusplus
extern "C" {
#endif

#define FIX_IMPORT(name)                                                       \
    __attribute__((import_module("fix"), import_name(name)))

FIX_IMPORT("is_eq") int32_t fix_is_eq(fix_handle value);
FIX_IMPORT("equals") int32_t fix_equals(fix_handle lhs, fix_handle rhs);
FIX_IMPORT("get_type") fix_value_type fix_get_type(fix_handle value);
FIX_IMPORT("get_data_type") fix_data_type fix_get_data_type(fix_handle data);
FIX_IMPORT("get_encode_type")
fix_encode_type fix_get_encode_type(fix_handle encode);
FIX_IMPORT("attach_blob")
void fix_attach_blob(fix_handle object, uint32_t memory);
FIX_IMPORT("attach_tree")
void fix_attach_tree(fix_handle object, uint32_t table);
FIX_IMPORT("detach_blob") void fix_detach_blob(uint32_t memory, int32_t copy);
FIX_IMPORT("detach_tree") void fix_detach_tree(uint32_t table, int32_t copy);
FIX_IMPORT("create_blob")
fix_handle fix_create_blob(uint32_t memory, uint32_t length);
FIX_IMPORT("create_blob_i32") fix_handle fix_create_blob_i32(uint32_t value);
FIX_IMPORT("create_blob_i64") fix_handle fix_create_blob_i64(uint64_t value);
FIX_IMPORT("create_tree")
fix_handle fix_create_tree(uint32_t table, uint32_t length);
FIX_IMPORT("create_tag") fix_handle fix_create_tag(uint32_t table);
FIX_IMPORT("create_ref") fix_handle fix_create_ref(fix_handle object);
FIX_IMPORT("create_identification_thunk")
fix_handle fix_create_identification_thunk(fix_handle data);
FIX_IMPORT("create_application_thunk")
fix_handle fix_create_application_thunk(fix_handle tree);
FIX_IMPORT("create_selection_thunk")
fix_handle fix_create_selection_thunk(fix_handle tree);
FIX_IMPORT("create_strict_encode")
fix_handle fix_create_strict_encode(fix_handle thunk);
FIX_IMPORT("create_shallow_encode")
fix_handle fix_create_shallow_encode(fix_handle thunk);
FIX_IMPORT("is_blob_obj") int32_t fix_is_blob_obj(fix_handle value);
FIX_IMPORT("is_object") int32_t fix_is_object(fix_handle value);
FIX_IMPORT("is_data") int32_t fix_is_data(fix_handle value);
FIX_IMPORT("is_tag") int32_t fix_is_tag(fix_handle value);
FIX_IMPORT("len") uint32_t fix_len(fix_handle value);

#undef FIX_IMPORT

#ifdef __cplusplus
}
#endif

#endif
