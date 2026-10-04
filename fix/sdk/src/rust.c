#include "fix.h"
#include "wasm.h"

typedef uint32_t (*fix_sdk_callback)(void *);
typedef uint32_t (*fix_sdk_child)(void *, uint32_t);

WASM_TABLE(1);
WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(1, 0);

enum { GROW_FAILED = 1, OUT_OF_BOUNDS, WRONG_TYPE, NOT_EQ, ALL_OCCUPIED = 6 };
static int active;
static fix_handle __attribute__((address_space(1))) focus, value;
static fix_handle __attribute__((address_space(1))) result;
static fix_handle null(void) { return __builtin_wasm_ref_null_extern(); }
uint32_t fix_sdk_blob(const void *bytes, uint32_t length) {
    uint32_t pages = (length >> 16) + ((length & 65535) != 0);
    uint32_t size = wasm_memory_size_1();
    if (pages > size && wasm_memory_grow_1(pages - size) < 0)
        return GROW_FAILED;
    wasm_memory_copy_0_to_1(0, bytes, length);
    value = fix_create_blob(1, length);
    return 0;
}
static uint32_t children(uint32_t length, uint32_t index, fix_sdk_child child,
                         void *context) {
    if (index == length) {
        fix_detach_tree(1, 0);
        uint32_t size = wasm_table_size_1();
        return length > size && wasm_table_grow_1(null(), length - size) < 0
                   ? GROW_FAILED
                   : 0;
    }
    uint32_t error = child(context, index);
    if (error)
        return error;
    fix_handle saved = value;
    value = null();
    error = children(length, index + 1, child, context);
    if (!error)
        wasm_table_set_1(index, saved);
    return error;
}
uint32_t fix_sdk_tree(uint32_t length, int tag, fix_sdk_child child,
                      void *context) {
    if (tag && !length)
        return OUT_OF_BOUNDS;
    uint32_t error = children(length, 0, child, context);
    if (error)
        return error;
    value = tag ? fix_create_tag(1) : fix_create_tree(1, length);
    wasm_table_fill_1(0, null(), length);
    return 0;
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
    fix_attach_tree(value, 1);
    value = wasm_table_get_1(index);
    fix_detach_tree(1, 0);
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
uint32_t fix_sdk_encode_type(uint32_t *out) {
    if (fix_get_type(value) != FIX_ENCODE)
        return WRONG_TYPE;
    *out = fix_get_encode_type(value);
    return 0;
}
uint32_t fix_sdk_len(uint32_t *out) {
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
uint32_t fix_sdk_is_eq(uint32_t *out) {
    *out = fix_is_eq(value);
    return 0;
}
uint32_t fix_sdk_equals(fix_sdk_callback rhs, void *context, int32_t *out) {
    fix_handle left = value;
    value = null();
    uint32_t error = rhs(context);
    if (error)
        return error;
    if (!fix_is_eq(left) || !fix_is_eq(value))
        return NOT_EQ;
    *out = fix_equals(left, value);
    value = null();
    return 0;
}
uint32_t fix_sdk_read(uint32_t offset, void *destination, uint32_t length) {
    if (!fix_is_object(value) || fix_get_data_type(value) != FIX_BLOB)
        return WRONG_TYPE;
    uint32_t size = fix_len(value);
    if (offset > size || length > size - offset)
        return OUT_OF_BOUNDS;
    fix_attach_blob(value, 1);
    wasm_memory_copy_1_to_0(destination, (const void *)(uintptr_t)offset,
                            length);
    fix_detach_blob(1, 0);
    value = null();
    return 0;
}
uint32_t fix_sdk_begin(void) {
    if (active)
        return ALL_OCCUPIED;
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
