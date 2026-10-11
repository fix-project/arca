#include "fix.h"
#include "wasm.h"

WASM_TABLE(2);

extern void fix_sdk_finish(void);
extern fix_handle fix_sdk_value(void);
extern void fix_sdk_restore(fix_handle);

uint32_t fix_json_cache_prepare(uint32_t length) {
    fix_handle null = __builtin_wasm_ref_null_extern();
    uint32_t size = wasm_table_size_2();
    if (length > size && wasm_table_grow_2(null, length - size) < 0)
        return 1;
    wasm_table_fill_2(0, null, length);
    return 0;
}

void fix_json_cache_load(uint32_t index) {
    fix_sdk_restore(wasm_table_get_2(index));
}

void fix_json_cache_save(uint32_t index) {
    fix_sdk_finish();
    fix_handle value = fix_sdk_value();
    wasm_table_set_2(index, value);
    fix_sdk_restore(value);
}
