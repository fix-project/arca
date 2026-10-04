#include "fix.h"
#include "wasm.h"

WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(1, 0);

WASM_MEMORY(2);
WASM_MEMORY_COPY(0, 2);
WASM_MEMORY_COPY(2, 0);
WASM_MEMORY_COPY(2, 2);
WASM_TABLE(2);
WASM_TABLE(3);
WASM_TABLE_COPY(2, 3);
WASM_TABLE_COPY(3, 3);

static fix_handle source[0], destination[0];
static void check(int condition) {
    if (!condition)
        __builtin_trap();
}

__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle input) {
    (void)input;
    check(wasm_table_grow(source, __builtin_wasm_ref_null_extern(), 4) == 0);
    check(wasm_table_grow(destination, __builtin_wasm_ref_null_extern(), 4) ==
          0);
    fix_handle number = fix_create_blob_i64(42);
    wasm_table_set(source, 1, number);
    wasm_table_copy(destination, source, 1, 2, 1);
    fix_handle copied = wasm_table_get(destination, 2);
    check(fix_get_type(copied) == FIX_OBJECT &&
          fix_get_data_type(copied) == FIX_BLOB);
    check(fix_is_eq(number) && fix_equals(number, fix_create_blob_i64(42)));
    check(!fix_equals(number, fix_create_blob_i64(43)));
    fix_handle reference = fix_create_ref(copied);
    check(fix_get_type(reference) == FIX_REF &&
          fix_get_data_type(reference) == FIX_BLOB);
    check(fix_is_eq(reference) && !fix_equals(number, reference));
    check(fix_equals(reference, fix_create_ref(fix_create_blob_i64(42))));
    fix_handle identification = fix_create_identification_thunk(reference);
    wasm_table_fill(destination, 0, number, 4);
    fix_handle tree = fix_create_tree(1, 4);
    check(fix_get_data_type(tree) == FIX_TREE && fix_len(tree) == 4);
    check(fix_is_eq(tree));
    fix_handle tree_ref = fix_create_ref(tree);
    fix_handle application = fix_create_application_thunk(tree_ref);
    fix_handle selection = fix_create_selection_thunk(tree_ref);
    check(fix_get_type(identification) == FIX_THUNK);
    check(fix_get_type(application) == FIX_THUNK &&
          fix_get_type(selection) == FIX_THUNK);
    fix_handle strict = fix_create_strict_encode(identification);
    check(!fix_is_eq(identification) && !fix_is_eq(strict));
    wasm_table_set(destination, 0, strict);
    check(!fix_is_eq(fix_create_tree(1, 1)));
    fix_handle shallow = fix_create_shallow_encode(application);
    check(fix_get_type(strict) == FIX_ENCODE &&
          fix_get_encode_type(strict) == FIX_STRICT);
    check(fix_get_type(shallow) == FIX_ENCODE &&
          fix_get_encode_type(shallow) == FIX_SHALLOW);

    fix_attach_tree(tree, 0);
    check(wasm_table_size(source) == 4);
    wasm_table_get(source, 3);
    fix_detach_tree(0, 1);
    wasm_table_set(source, 3, fix_create_blob_i64(99));
    fix_attach_tree(tree, 0);
    number = wasm_table_get(source, 3);
    fix_detach_tree(0, 0);
    check(wasm_table_size(source) == 0);

    fix_attach_blob(number, 1);
    uint64_t integer;
    wasm_memory_copy_1_to_0(&integer, 0, 8);
    check(integer == 42);
    fix_detach_blob(1, 0);
    check(wasm_memory_size_1() == 0);
    check(wasm_memory_grow_1(2) == 0);
    uint8_t bytes[64];
    for (uint32_t i = 0; i < 64; ++i)
        bytes[i] = i;
    wasm_memory_copy_0_to_1((void *)65520, bytes, 64);
    fix_handle large = fix_create_blob(1, 65584);
    fix_attach_blob(large, 1);
    uint8_t read[32];
    wasm_memory_copy_1_to_0(read, (void *)65536, 32);
    for (uint32_t i = 0; i < 32; ++i)
        check(read[i] == i + 16);
    fix_detach_blob(1, 1);
    bytes[0] = 255;
    wasm_memory_copy_0_to_1((void *)65536, bytes, 1);
    fix_attach_blob(large, 1);
    wasm_memory_copy_1_to_0(read, (void *)65536, 1);
    check(read[0] == 16);
    fix_detach_blob(1, 0);
    fix_detach_tree(1, 0);

    check(wasm_memory_size_2() == 0 && wasm_memory_grow_2(2) == 0);
    for (uint32_t i = 0; i < 64; ++i)
        bytes[i] = i;
    wasm_memory_copy_0_to_2((void *)65520, bytes, 64);
    wasm_memory_copy_2_to_2((void *)65521, (void *)65520, 63);
    wasm_memory_copy_2_to_0(bytes, (void *)65520, 64);
    check(bytes[0] == 0);
    for (uint32_t i = 1; i < 64; ++i)
        check(bytes[i] == i - 1);
    wasm_memory_fill_2((void *)65535, 0x5a, 4);
    wasm_memory_copy_2_to_0(bytes, (void *)65534, 6);
    check(bytes[0] == 13 && bytes[5] == 18);
    for (uint32_t i = 1; i < 5; ++i)
        check(bytes[i] == 0x5a);

    check(wasm_table_size_2() == 0 && wasm_table_size_3() == 0);
    check(wasm_table_grow_2(number, 4) == 0);
    check(wasm_table_grow_3(__builtin_wasm_ref_null_extern(), 4) == 0);
    wasm_table_set_2(1, fix_create_blob_i64(99));
    wasm_table_copy_2_to_3(1, 0, 3);
    wasm_table_copy_3_to_3(2, 1, 2);
    fix_attach_blob(wasm_table_get_3(3), 1);
    wasm_memory_copy_1_to_0(&integer, 0, 8);
    check(integer == 99);
    fix_detach_blob(1, 0);
    wasm_table_fill_3(0, number, 4);
    fix_attach_blob(wasm_table_get_3(3), 1);
    wasm_memory_copy_1_to_0(&integer, 0, 8);
    check(integer == 42);
    fix_detach_blob(1, 0);
    fix_detach_blob(2, 0);
    fix_detach_tree(2, 0);
    fix_detach_tree(3, 0);
    return number;
}
