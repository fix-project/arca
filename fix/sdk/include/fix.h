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
typedef enum fix_thunk_type {
    FIX_APPLICATION = 0,
    FIX_SELECTION = 1,
    FIX_IDENTIFICATION = 2
} fix_thunk_type;

#ifdef __cplusplus
extern "C" {
#endif

#define FIX_IMPORT(name)                                                       \
    __attribute__((import_module("fix"), import_name(name)))

/* Invalidates every existing externref. Preserves keep and all Tables and
 * attachments; returns a fresh reference to keep. */
FIX_IMPORT("invalidate_externrefs")
fix_handle fix_invalidate_externrefs(fix_handle keep);

FIX_IMPORT("is_comparable") int32_t fix_is_comparable(fix_handle value);
FIX_IMPORT("is_value") int32_t fix_is_value(fix_handle value);
FIX_IMPORT("unsound_handle_bytes")
fix_handle fix_unsound_handle_bytes(fix_handle value);
/* Requires privileged execution. Copies the handle's 32-byte encoding into
 * writable memory 0. Local names are lifted before the copy. */
FIX_IMPORT("read_handle_bytes")
void fix_read_handle_bytes(fix_handle value, void *destination);
/* Requires privileged execution. Compares exact encodings after lifting local
 * names. */
FIX_IMPORT("same_handle_bytes")
uint32_t fix_same_handle_bytes(fix_handle lhs, fix_handle rhs);
FIX_IMPORT("equals") int32_t fix_equals(fix_handle lhs, fix_handle rhs);
FIX_IMPORT("get_type") fix_value_type fix_get_type(fix_handle value);
FIX_IMPORT("get_data_type") fix_data_type fix_get_data_type(fix_handle data);
/* Requires privileged execution. Accepts a Thunk or Encode. */
FIX_IMPORT("get_thunk_type")
fix_thunk_type fix_get_thunk_type(fix_handle thunk);
FIX_IMPORT("get_encode_type")
fix_encode_type fix_get_encode_type(fix_handle encode);
/* Copies up to length bytes into writable memory 0 and returns the number
 * copied. Stops at the end of the Blob. Invalid types, offsets past the end,
 * and destination ranges outside memory 0 trap. */
FIX_IMPORT("read_blob")
uint32_t fix_read_blob(fix_handle blob, uint32_t offset, void *destination,
                       uint32_t length);
FIX_IMPORT("attach_blob")
void fix_attach_blob(fix_handle object, uint32_t memory);
FIX_IMPORT("get_tree_element")
fix_handle fix_get_tree_element(fix_handle object, uint32_t index);
FIX_IMPORT("attach_tree")
void fix_attach_tree(fix_handle object, uint32_t table);
/* Requires privileged execution. Attaches the Tree's packed 32-byte handles
 * as read-only bytes in the selected Memory. */
FIX_IMPORT("attach_tree_bytes")
void fix_attach_tree_bytes(fix_handle object, uint32_t memory);
FIX_IMPORT("detach_blob") void fix_detach_blob(uint32_t memory, int32_t copy);
FIX_IMPORT("detach_tree") void fix_detach_tree(uint32_t table, int32_t copy);
FIX_IMPORT("create_blob")
fix_handle fix_create_blob(uint32_t memory, uint32_t length);
FIX_IMPORT("create_blob_i32") fix_handle fix_create_blob_i32(uint32_t value);
FIX_IMPORT("create_blob_i64") fix_handle fix_create_blob_i64(uint64_t value);
FIX_IMPORT("create_tree")
fix_handle fix_create_tree(uint32_t table, uint32_t length);
FIX_IMPORT("create_tag")
/* Field zero is a comparable Object or Ref identifying the current combination
 * or one of its callable prefixes. All fields are preserved. */
fix_handle fix_create_tag(uint32_t table);
/* Requires scheduler mode and a configured trusted tagger. Preserves all fields
 * without adding an author. The first field is the author. */
FIX_IMPORT("create_tag_unchecked")
fix_handle fix_create_tag_unchecked(fix_handle tree);
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
FIX_IMPORT("len") uint64_t fix_len(fix_handle value);
FIX_IMPORT("eval") fix_handle fix_eval(fix_handle value);
FIX_IMPORT("effect") fix_handle fix_effect(fix_handle combination);
/* Emits non-normative metadata. The application cannot read advice or receive
 * scheduler data in reply. Keys are UTF-8; payloads are arbitrary bytes. */
FIX_IMPORT("advise")
void fix_advise(fix_handle value, const uint8_t *key, uint32_t key_length,
                const uint8_t *payload, uint32_t payload_length);
/* Scheduler coordination, available only in scheduler mode. Arguments are
 * entries 0..count in an externref Table. Writes a status to memory 0. */
FIX_IMPORT("coordinate")
fix_handle fix_coordinate(const uint8_t *name, uint32_t name_length,
                          uint32_t table, uint32_t count, uint64_t *status);
FIX_IMPORT("return_cc") fix_handle fix_return_cc(fix_handle request);
FIX_IMPORT("call_cc") fix_handle fix_call_cc(fix_handle callable);

#undef FIX_IMPORT

#ifdef __cplusplus
}
#endif

#endif
