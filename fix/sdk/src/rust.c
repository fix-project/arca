#include "fix.h"
#include "wasm.h"

typedef uint32_t (*fix_sdk_callback)(void *);
typedef uint32_t (*fix_sdk_child)(void *, uint32_t);

WASM_TABLE(1);
WASM_TABLE(2);
WASM_MEMORY(1);
WASM_MEMORY(2);
WASM_MEMORY_COPY(0, 2);
WASM_MEMORY_COPY(2, 0);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(1, 0);

WASM_POSTPROCESSOR_IMPORT("table_get")
fix_handle wasm_table_get_dynamic(uint32_t table, uint32_t index);
WASM_POSTPROCESSOR_IMPORT("memory_copy_to_0")
void wasm_memory_read(uint32_t memory, void *destination, uint32_t offset,
                      uint32_t length);

enum { GROW_FAILED = 1, OUT_OF_BOUNDS, WRONG_TYPE, NOT_EQ, BUSY = 6 };
static int active;
static uint32_t free_capture = UINT32_MAX;
static fix_handle __attribute__((address_space(1))) focus, value;
static fix_handle __attribute__((address_space(1))) result;
static fix_handle null(void) { return __builtin_wasm_ref_null_extern(); }
uint32_t fix_sdk_invalidate_externrefs(void) {
    if (!active)
        return BUSY;
    focus = fix_invalidate_externrefs(focus);
    return 0;
}
static uint32_t capture(fix_handle input, uint32_t *out) {
    uint32_t index = free_capture;
    if (index == UINT32_MAX) {
        index = wasm_table_size_2();
        uint32_t pages = (index >> 14) + 1;
        uint32_t size = wasm_memory_size_2();
        if (pages > size && wasm_memory_grow_2(pages - size) < 0)
            return GROW_FAILED;
        if (wasm_table_grow_2(null(), 1) < 0)
            return GROW_FAILED;
    } else {
        wasm_memory_copy_2_to_0(&free_capture,
                                (void *)(uintptr_t)(index * sizeof(uint32_t)),
                                sizeof(free_capture));
    }
    wasm_table_set_2(index, input);
    *out = index;
    return 0;
}
void fix_sdk_release_capture(uint32_t index) {
    wasm_table_set_2(index, null());
    wasm_memory_copy_0_to_2((void *)(uintptr_t)(index * sizeof(uint32_t)),
                            &free_capture, sizeof(free_capture));
    free_capture = index;
}
uint32_t fix_sdk_capture(uint32_t index) {
    if (index >= wasm_table_size_2())
        return OUT_OF_BOUNDS;
    value = wasm_table_get_2(index);
    return 0;
}
static uint32_t transfer(uint32_t *out, int effect) {
    if (!active)
        return BUSY;
    value = effect ? fix_effect(value) : fix_eval(value);
    uint32_t error = capture(value, out);
    value = null();
    return error;
}
uint32_t fix_sdk_eval(uint32_t *out) { return transfer(out, 0); }
uint32_t fix_sdk_effect(uint32_t *out) { return transfer(out, 1); }
uint32_t fix_sdk_keep(uint32_t *out) {
    uint32_t error = capture(value, out);
    value = null();
    return error;
}
uint32_t fix_sdk_handle_bytes(uint32_t *out) {
    value = fix_unsound_handle_bytes(value);
    return fix_sdk_keep(out);
}
uint32_t fix_sdk_read_handle_bytes(uint8_t *out) {
    if (!active)
        return BUSY;
    fix_read_handle_bytes(value, out);
    value = null();
    return 0;
}
uint32_t fix_sdk_tag_unchecked(uint32_t *out) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    value = fix_create_tag_unchecked(value);
    return fix_sdk_keep(out);
}
uint32_t fix_sdk_blob(const void *bytes, uint32_t length) {
    uint32_t pages = (length >> 16) + ((length & 65535) != 0);
    uint32_t size = wasm_memory_size_1();
    if (pages > size && wasm_memory_grow_1(pages - size) < 0)
        return GROW_FAILED;
    wasm_memory_copy_0_to_1(0, bytes, length);
    value = fix_create_blob(1, length);
    return 0;
}
static uint32_t children(uint32_t length, uint32_t index, int tag,
                         fix_sdk_child child, void *context) {
    if (index == length) {
        uint32_t size = wasm_table_size_1();
        if (tag && size != length) {
            fix_detach_tree(1, 0);
            size = 0;
        }
        return length > size && wasm_table_grow_1(null(), length - size) < 0
                   ? GROW_FAILED
                   : 0;
    }
    uint32_t error = child(context, index);
    if (error)
        return error;
    fix_handle saved = value;
    value = null();
    error = children(length, index + 1, tag, child, context);
    if (!error)
        wasm_table_set_1(index, saved);
    return error;
}
uint32_t fix_sdk_tree(uint32_t length, int tag, fix_sdk_child child,
                      void *context) {
    if (tag && !length)
        return OUT_OF_BOUNDS;
    uint32_t error = children(length, 0, tag, child, context);
    if (error)
        return error;
    value = tag ? fix_create_tag(1) : fix_create_tree(1, length);
    wasm_table_fill_1(0, null(), length);
    return 0;
}
uint32_t fix_sdk_advise(const uint8_t *key, uint32_t key_length,
                        const uint8_t *payload, uint32_t payload_length) {
    if (!active)
        return BUSY;
    fix_advise(value, key, key_length, payload, payload_length);
    return 0;
}
uint32_t fix_sdk_coordinate(const uint8_t *name, uint32_t name_length,
                            uint32_t length, fix_sdk_child child, void *context,
                            uint64_t *reply_status, uint32_t *out) {
    if (!active)
        return BUSY;
    uint32_t error = children(length, 0, 0, child, context);
    if (error)
        return error;
    value = fix_coordinate(name, name_length, 1, length, reply_status);
    wasm_table_fill_1(0, null(), length);
    error = capture(value, out);
    value = null();
    return error;
}
uint32_t fix_sdk_reference(void) {
    if (fix_get_type(value) != FIX_OBJECT)
        return WRONG_TYPE;
    value = fix_create_ref(value);
    return 0;
}
uint32_t fix_sdk_identification(void) {
    if (!fix_is_data(value))
        return WRONG_TYPE;
    value = fix_create_identification_thunk(value);
    return 0;
}
uint32_t fix_sdk_application(void) {
    if (!fix_is_data(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    value = fix_create_application_thunk(value);
    return 0;
}
uint32_t fix_sdk_selection(void) {
    if (!fix_is_data(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    value = fix_create_selection_thunk(value);
    return 0;
}
uint32_t fix_sdk_strict(void) {
    if (fix_get_type(value) != FIX_THUNK)
        return WRONG_TYPE;
    value = fix_create_strict_encode(value);
    return 0;
}
uint32_t fix_sdk_shallow(void) {
    if (fix_get_type(value) != FIX_THUNK)
        return WRONG_TYPE;
    value = fix_create_shallow_encode(value);
    return 0;
}
uint32_t fix_sdk_entry(uint32_t index) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    if (index >= fix_len(value))
        return OUT_OF_BOUNDS;
    value = fix_get_tree_element(value, index);
    return 0;
}
uint32_t fix_sdk_value_type(uint32_t *out) {
    *out = fix_get_type(value);
    return 0;
}
uint32_t fix_sdk_data_type(uint32_t *out) {
    if (!fix_is_data(value))
        return WRONG_TYPE;
    *out = fix_get_data_type(value);
    return 0;
}
uint32_t fix_sdk_thunk_type(uint32_t *out) {
    fix_value_type type = fix_get_type(value);
    if (type != FIX_THUNK && type != FIX_ENCODE)
        return WRONG_TYPE;
    *out = fix_get_thunk_type(value);
    return 0;
}
uint32_t fix_sdk_encode_type(uint32_t *out) {
    if (fix_get_type(value) != FIX_ENCODE)
        return WRONG_TYPE;
    *out = fix_get_encode_type(value);
    return 0;
}
uint32_t fix_sdk_len(uint64_t *out) {
    if (!fix_is_data(value))
        return WRONG_TYPE;
    *out = fix_len(value);
    return 0;
}
uint32_t fix_sdk_is_tag(uint32_t *out) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    *out = fix_is_tag(value);
    return 0;
}
uint32_t fix_sdk_is_comparable(uint32_t *out) {
    *out = fix_is_comparable(value);
    return 0;
}
uint32_t fix_sdk_is_value(uint32_t *out) {
    *out = fix_is_value(value);
    return 0;
}
uint32_t fix_sdk_equals(fix_sdk_callback rhs, void *context, int32_t *out) {
    fix_handle left = value;
    value = null();
    uint32_t error = rhs(context);
    if (error)
        return error;
    if (!fix_is_comparable(left) || !fix_is_comparable(value))
        return NOT_EQ;
    *out = fix_equals(left, value);
    value = null();
    return 0;
}
uint32_t fix_sdk_attach_table(uint32_t table) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    fix_attach_tree(value, table);
    value = null();
    return 0;
}
uint32_t fix_sdk_attach_memory(uint32_t memory) {
    if (!fix_is_blob_obj(value))
        return WRONG_TYPE;
    fix_attach_blob(value, memory);
    value = null();
    return 0;
}
uint32_t fix_sdk_attach_tree_bytes(uint32_t memory) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_TREE)
        return WRONG_TYPE;
    fix_attach_tree_bytes(value, memory);
    value = null();
    return 0;
}
void fix_sdk_detach_table(uint32_t table) { fix_detach_tree(table, 0); }
void fix_sdk_detach_memory(uint32_t memory) { fix_detach_blob(memory, 0); }
void fix_sdk_table_entry(uint32_t table, uint32_t index) {
    value = wasm_table_get_dynamic(table, index);
}
void fix_sdk_memory_read(uint32_t memory, uint32_t offset, void *destination,
                         uint32_t length) {
    wasm_memory_read(memory, destination, offset, length);
}
uint32_t fix_sdk_read(uint32_t offset, void *destination, uint32_t length) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_BLOB)
        return WRONG_TYPE;
    uint64_t size = fix_len(value);
    if (offset > size || length > size - offset)
        return OUT_OF_BOUNDS;
    fix_read_blob(value, offset, destination, length);
    value = null();
    return 0;
}
uint32_t fix_sdk_read_exact(void *destination, uint32_t length) {
    if (!fix_is_blob_obj(value) || fix_len(value) != length)
        return WRONG_TYPE;
    return fix_sdk_read(0, destination, length);
}
uint32_t fix_sdk_begin(void) {
    if (active)
        return BUSY;
    if (!fix_is_object(focus) || fix_get_data_type(focus) != FIX_TREE)
        return WRONG_TYPE;
    active = 1;
    return 0;
}
void fix_sdk_root(void) { value = focus; }
void fix_sdk_reset(void) {
    active = 0;
    value = focus = null();
    fix_detach_blob(1, 0);
    fix_detach_tree(1, 0);
}
void fix_sdk_finish(void) {
    result = value;
    value = null();
}
void fix_sdk_restore(fix_handle input) { value = input; }
void fix_sdk_input(fix_handle input) {
    if (!active) {
        focus = input;
        result = null();
    }
}
fix_handle fix_sdk_value(void) {
    fix_handle output = result;
    result = null();
    return output;
}

extern uint32_t _fix_apply_inner(void);
__attribute__((export_name("_fix_apply"))) fix_handle
_fix_apply(fix_handle input) {
    fix_sdk_input(input);
    if (fix_sdk_begin())
        __builtin_trap();
    uint32_t error = _fix_apply_inner();
    fix_handle output = fix_sdk_value();
    fix_sdk_reset();
    if (error)
        __builtin_trap();
    return output;
}
