#include "fix.h"
#include "wasm.h"

WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);

__attribute__((export_name("_fix_apply"))) fix_handle apply(fix_handle combination) {
    (void)combination;
    const char bytes[] = "hello, world";
    if (wasm_memory_grow_1(1) < 0)
        __builtin_trap();
    wasm_memory_copy_0_to_1(0, bytes, sizeof(bytes) - 1);
    return fix_create_blob(1, sizeof(bytes) - 1);
}
