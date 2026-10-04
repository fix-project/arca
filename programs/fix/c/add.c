#include "fix.h"
#include "wasm.h"

WASM_MEMORY_COPY(1, 0);

static fix_handle values[0];

static fix_handle entry(fix_handle tree, uint32_t index) {
    fix_attach_tree(tree, 0);
    fix_handle value = wasm_table_get(values, index);
    fix_detach_tree(0, 0);
    return value;
}

static uint64_t integer(fix_handle blob) {
    if (!fix_is_blob_obj(blob) || fix_len(blob) != 8)
        __builtin_trap();
    uint64_t value;
    fix_attach_blob(blob, 1);
    wasm_memory_copy_1_to_0(&value, 0, sizeof(value));
    fix_detach_blob(1, 0);
    return value;
}

__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle combination) {
    uint64_t left = integer(entry(combination, 1));
    uint64_t right = integer(entry(combination, 2));
    return fix_create_blob_i64(left + right);
}
