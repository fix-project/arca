#include "fix.h"
#include "wasm.h"
WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(1, 0);
static void check(int condition) {
    if (!condition)
        __builtin_trap();
}
__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle input) {
    (void)input;
    fix_handle scalar = fix_create_blob_i64(42);
    fix_handle reference = fix_create_ref(scalar);
    check(fix_get_type(scalar) == FIX_OBJECT &&
          fix_get_data_type(scalar) == FIX_BLOB);
    check(fix_get_type(reference) == FIX_REF &&
          fix_get_data_type(reference) == FIX_BLOB);
    check(fix_is_comparable(scalar) && !fix_equals(scalar, reference));
    fix_handle fields[] = {scalar, reference, scalar};
    fix_handle saved = fix_create_tree(fields, 3), copied[3];
    check(fix_access_tree(saved, 0, copied, 3) == 3);
    check(fix_equals(copied[0], scalar) && fix_equals(copied[1], reference));
    fix_close(copied[2]);
    fix_handle reused = fix_clone(scalar);
    check(reused == copied[2]);
    fix_close(reused);
    fix_handle live[260];
    for (uint32_t i = 0; i < 260; i++)
        live[i] = fix_clone(scalar);
    uint32_t boundary = 0;
    while (live[boundary] % 128)
        ++boundary;
    fix_handle closed = live[boundary];
    fix_close(closed);
    live[boundary] = fix_clone(scalar);
    check(live[boundary] == closed);
    for (uint32_t i = 0; i < 260; i++) {
        check(fix_equals(live[i], scalar));
        fix_close(live[i]);
    }
    fix_handle identification = fix_create_identification_thunk(reference);
    fix_handle strict = fix_create_strict_encode(identification),
               shallow = fix_create_shallow_encode(identification);
    check(fix_get_type(strict) == FIX_ENCODE &&
          fix_get_encode_type(strict) == FIX_STRICT);
    check(fix_get_encode_type(shallow) == FIX_SHALLOW &&
          !fix_is_comparable(strict));
    check(fix_attach_blob(1, scalar) == 8);
    uint64_t integer;
    wasm_memory_copy_1_to_0(&integer, 0, 8);
    check(integer == 42);
    fix_detach_blob(1, 0);
    check(wasm_memory_size_1() == 0 && wasm_memory_grow_1(2) == 0);
    uint8_t bytes[64];
    for (uint32_t i = 0; i < 64; i++)
        bytes[i] = i;
    wasm_memory_copy_0_to_1((void *)65520, bytes, 64);
    fix_handle original = fix_create_blob(1, 65584);
    check(fix_attach_blob(1, original) == 65584);
    fix_detach_blob(1, 1);
    uint8_t changed = 99;
    wasm_memory_copy_0_to_1((void *)65520, &changed, 1);
    fix_handle modified = fix_create_blob(1, 65584);
    uint8_t actual;
    check(fix_access_blob(original, 65520, &actual, 1) == 1 && actual == 0);
    check(fix_access_blob(modified, 65520, &actual, 1) == 1 && actual == 99);
    fix_detach_blob(1, 0);
    fix_close(original);
    fix_close(modified);
    fix_close(saved);
    fix_close(copied[0]);
    fix_close(copied[1]);
    return scalar;
}
