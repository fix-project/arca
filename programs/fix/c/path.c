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
    return fix_len(tree);
}

static fix_handle field(fix_handle descriptor, uint32_t index) {
    if (tree_length(descriptor) != 3)
        __builtin_trap();
    return entry(descriptor, index);
}

__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle combination) {
    fix_handle descriptor = entry(combination, 1);
    fix_handle components = entry(combination, 2);
    uint32_t count = tree_length(components);
    if (!tree_length(descriptor))
        return descriptor;
    if (tree_length(descriptor) != 3)
        __builtin_trap();
    for (uint32_t index = 0; index < count; ++index) {
        fix_handle wanted = entry(components, index);
        fix_handle children = field(descriptor, 2);
        uint32_t child_count = tree_length(children);
        int found = 0;
        for (uint32_t child_index = 0; child_index < child_count; ++child_index) {
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
