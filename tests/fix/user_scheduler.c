#include "fix.h"
#include "wasm.h"

WASM_MEMORY(1);
WASM_MEMORY_COPY(0, 1);
WASM_MEMORY_COPY(2, 0);

static fix_handle values[0];

static void check(int condition) {
    if (!condition)
        __builtin_trap();
}

static uint64_t integer(fix_handle blob) {
    check(fix_is_blob_obj(blob) && fix_len(blob) == 8);
    uint64_t value;
    fix_attach_blob(blob, 2);
    wasm_memory_copy_2_to_0(&value, 0, sizeof(value));
    fix_detach_blob(2, 0);
    return value;
}

static fix_handle request(const char *name, uint32_t arguments, fix_handle x,
                          fix_handle y) {
    uint32_t length = 0;
    while (name[length])
        ++length;
    wasm_memory_copy_0_to_1(0, name, length);
    wasm_table_set(values, 0, fix_create_blob(1, length));
    wasm_table_set(values, 1, x);
    wasm_table_set(values, 2, y);
    return fix_effect(fix_create_tree(0, arguments + 1));
}

static fix_handle reply(fix_handle result, uint64_t status) {
    check(fix_is_object(result) && fix_len(result) == 2);
    check(integer(fix_get_tree_element(result, 0)) == status);
    return fix_get_tree_element(result, 1);
}

static fix_handle select(fix_handle target, uint64_t index) {
    wasm_table_set(values, 0, target);
    wasm_table_set(values, 1, fix_create_blob_i64(index));
    return fix_create_tree(0, 2);
}

static fix_handle application(fix_handle callable, fix_handle argument) {
    wasm_table_set(values, 0, fix_create_blob_i64(0));
    wasm_table_set(values, 1, callable);
    wasm_table_set(values, 2, argument);
    return fix_create_tree(0, 3);
}

__attribute__((export_name("_fix_apply"))) fix_handle
apply(fix_handle combination) {
    fix_handle zero = fix_create_blob_i64(0);
    check(wasm_memory_grow_1(1) >= 0);
    check(wasm_table_grow(values, zero, 6) >= 0);
    fix_handle root = fix_get_tree_element(combination, 3);
    uint64_t worker =
        integer(reply(request("worker_config", 1, zero, zero), 0));
    uint64_t workers = integer(
        reply(request("worker_config", 1, fix_create_blob_i64(1), zero), 0));
    check(worker < workers);
    if (worker != 0) {
        for (;;)
            reply(request("worker_yield", 0, zero, zero), 0);
    }
    check(integer(reply(
              request("worker_config", 1, fix_create_blob_i64(2), zero),
              0)) == 0);
    fix_handle queue_reply =
        request("queue_create", 1, fix_create_blob_i64(1), zero);
    if (integer(fix_get_tree_element(queue_reply, 0)) == 2) {
        for (;;)
            reply(request("worker_yield", 0, zero, zero), 0);
    }
    fix_handle queue = reply(queue_reply, 0);
    check(integer(reply(request("queue_count", 0, zero, zero), 0)) ==
          1);
    reply(request("queue_pop", 1, queue, zero), 1);
    reply(request("queue_push", 2, queue, root), 0);
    reply(request("queue_push", 2, queue, root), 2);
    check(fix_equals(reply(request("queue_pop", 1, queue, zero), 0), root));
    reply(request("queue_pop", 1, fix_create_blob_i64(1), zero), 4);
    reply(request("queue_create", 1, zero, zero), 5);
    for (uint64_t i = 0; i < workers; ++i)
        reply(
            request("queue_create", 1, fix_create_blob_i64(1), zero),
            0);
    reply(request("queue_create", 1, fix_create_blob_i64(1), zero),
          2);
    reply(request("worker_config", 1, fix_create_blob_i32(0), zero), 5);
    reply(request("worker_config", 1, fix_create_blob_i64(9), zero), 5);
    reply(request("unknown", 0, zero, zero), 5);
    reply(request("worker_yield", 0, zero, zero), 0);

    reply(request("callable_apply", 1, zero, zero), 5);
    reply(request("storage_select", 1, zero, zero), 5);
    fix_handle applied = reply(request("callable_apply", 1, root, zero), 0);
    check(fix_is_tag(applied));
    check(fix_len(fix_get_tree_element(applied, 0)) == 0);
    applied = reply(request("storage_put", 1, applied, zero), 0);
    wasm_table_set(values, 3, applied);
    fix_handle selection = select(root, 2);
    fix_handle selected =
        reply(request("storage_select", 1, selection, zero), 0);
    check(fix_is_tag(selected));
    wasm_table_set(values, 4, selected);
    fix_handle canonical =
        reply(request("storage_canonicalize", 1, root, zero), 0);
    check(fix_is_tag(canonical));
    fix_handle privileged =
        reply(request("callable_invoke", 1, root, zero), 0);
    check(fix_equals(privileged, root));
    fix_handle identified = reply(
        request("storage_identify", 1, fix_create_ref(root), zero), 0);
    check(fix_equals(identified, root));
    fix_handle pending =
        fix_create_strict_encode(fix_create_application_thunk(root));
    reply(request("storage_eval", 1, pending, zero), 5);
    fix_handle evaluated = reply(request("storage_eval", 1, root, zero), 0);
    check(fix_is_tag(evaluated));
    wasm_table_set(values, 5, evaluated);

    fix_handle program = fix_get_tree_element(root, 3);
    fix_handle captured = reply(request("callable_invoke", 1,
                                        application(program, zero), zero),
                                0);
    fix_handle continuation = fix_get_tree_element(captured, 1);
    for (uint64_t value = 7; value <= 41; value += 34) {
        fix_handle resumed =
            application(continuation, fix_create_blob_i64(value));
        check(integer(reply(
                  request("callable_invoke", 1, resumed, zero),
                  0)) == value + 1);
    }

    applied = wasm_table_get(values, 3);
    selected = wasm_table_get(values, 4);
    evaluated = wasm_table_get(values, 5);
    wasm_table_set(values, 0, applied);
    wasm_table_set(values, 1, selected);
    wasm_table_set(values, 2, canonical);
    wasm_table_set(values, 3, privileged);
    wasm_table_set(values, 4, identified);
    wasm_table_set(values, 5, evaluated);
    fix_handle result = fix_create_tree(0, 6);
    request("worker_finish", 1, result, zero);
    __builtin_trap();
}
