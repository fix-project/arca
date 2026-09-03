#include "utils.h"

static externref __attribute__((address_space(1))) combination_global;

static externref create_thunk(uint16_t meta, externref value) {
    switch (THUNK_TAG(meta)) {
    case IDENTIFICATION: return fixpoint_create_identification_thunk(value);
    case APPLICATION: return fixpoint_create_application_thunk(value);
    case SELECTION: return fixpoint_create_selection_thunk(value);
    default: __builtin_unreachable();
    }
}

static externref create_encode(uint16_t meta, externref value) {
    switch (ENCODE_TAG(meta)) {
    case STRICT: return fixpoint_create_strict_encode(value);
    case SHALLOW: return fixpoint_create_shallow_encode(value);
    default: __builtin_unreachable();
    }
}

static externref resolve(const struct RustHandle* handle) {
    externref value;

    switch (PRODUCER_TAG(handle->meta)) {
    case COMBINATION: value = combination_global; break;
    case TABLE_GET: value = wasm_table_get(handle->index, handle->entry); break;
    case CREATE_BLOB: value = fixpoint_create_blob(handle->index, handle->entry); break;
    case CREATE_TREE: value = fixpoint_create_tree(handle->index, handle->entry); break;
    default: __builtin_unreachable();
    }

    switch (HANDLE_TAG(handle->meta)) {
    case OBJECT: return value;
    case REF: return fixpoint_create_ref(value);
    case THUNK:  return create_thunk(handle->meta, value);
    case ENCODE: return create_encode(handle->meta, create_thunk(handle->meta, value));
    default: __builtin_unreachable();
    }
}

void util_attach_blob(uint32_t memory_index, const struct RustHandle* handle) {
    fixpoint_attach_blob(memory_index, resolve(handle));
}

void util_attach_tree(uint32_t table_index, const struct RustHandle* handle) {
    fixpoint_attach_tree(table_index, resolve(handle));
}

uint32_t util_len(const struct RustHandle* handle) {
    return fixpoint_len(resolve(handle));
}

void util_table_set(uint32_t table_index, uint32_t entry_index, const struct RustHandle* handle) {
    wasm_table_set(table_index, entry_index, resolve(handle));
}

static const struct RustHandle combination = {
    .meta = (COMBINATION << 12) | (OBJECT << 10) | (1 << 6),
};

__attribute__((export_name("_fixpoint_apply")))
externref fixpoint_apply(externref input) {
    combination_global = input;
    struct RustHandle output = _fixpoint_apply_inner(combination);
    return resolve(&output);
}