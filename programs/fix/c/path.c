#include "fix.h"
#include "wasm.h"

static fix_handle values[0];

static fix_handle entry(fix_handle tree, uint32_t index) {
    fix_attach_tree(tree, 0);
    fix_handle value = wasm_table_get(values, index);
    fix_detach_tree(0, 0);
    return value;
}

static uint32_t tree_length(fix_handle tree) {
    if (!fix_is_object(tree) || fix_is_blob_obj(tree))
        __builtin_trap();
    uint64_t length = fix_len(tree);
    if (length > UINT32_MAX)
        __builtin_trap();
    return (uint32_t)length;
}

static fix_handle field(fix_handle descriptor, uint32_t index) {
    if (tree_length(descriptor) != 3)
        __builtin_trap();
    return entry(descriptor, index);
}

static fix_handle tree3(fix_handle a, fix_handle b, fix_handle c) {
    wasm_table_grow(values, __builtin_wasm_ref_null_extern(), 3);
    wasm_table_set(values, 0, a);
    wasm_table_set(values, 1, b);
    wasm_table_set(values, 2, c);
    return fix_create_tree(0, 3);
}

static fix_handle resume(fix_handle combination, fix_handle descriptor,
                         fix_handle components) {
    fix_handle metadata = entry(combination, 0);
    fix_handle procedure = entry(combination, 1);
    wasm_table_grow(values, __builtin_wasm_ref_null_extern(), 4);
    wasm_table_set(values, 0, metadata);
    wasm_table_set(values, 1, procedure);
    wasm_table_set(values, 2, descriptor);
    wasm_table_set(values, 3, components);
    return fix_create_application_thunk(fix_create_tree(0, 4));
}

__attribute__((export_name("_fix_apply"))) fix_handle
apply(fix_handle combination) {
    fix_handle descriptor = entry(combination, 2);
    fix_handle components = entry(combination, 3);
    uint32_t count = tree_length(components);
    if (!tree_length(descriptor))
        return descriptor;
    if (tree_length(descriptor) != 3)
        __builtin_trap();
    for (uint32_t index = 0; index < count; ++index) {
        fix_handle wanted = entry(components, index);
        fix_handle children = field(descriptor, 2);
        if (fix_get_type(children) == FIX_REF) {
            fix_handle loaded = fix_create_strict_encode(
                fix_create_identification_thunk(children));
            descriptor =
                tree3(field(descriptor, 0), field(descriptor, 1), loaded);
            components = fix_create_strict_encode(fix_create_selection_thunk(
                tree3(components, fix_create_blob_i64(index),
                      fix_create_blob_i64(count))));
            return resume(combination, descriptor, components);
        }
        uint32_t child_count = tree_length(children);
        int found = 0;
        for (uint32_t child_index = 0; child_index < child_count;
             ++child_index) {
            fix_handle child = entry(children, child_index);
            if (fix_equals(field(child, 0), wanted)) {
                descriptor = child;
                found = 1;
                break;
            }
        }
        if (!found)
            return fix_create_tree(0, 0);
    }
    return descriptor;
}
