#include <stdint.h>

typedef __externref_t externref;
static externref __attribute__((address_space(1))) combination_global;

enum producer {
    COMBINATION = 0,
    TABLE_GET = 1,
    CREATE_BLOB = 2,
    CREATE_TREE = 3,
};

struct RustHandle {
    uint8_t name[24];
    union {
        uint64_t body;
        struct {
            uint32_t entry;
            uint8_t producer;
            uint8_t index;
            uint16_t meta;
        };
    };
};

// Imports
__attribute__((import_module("fixpoint"), import_name("create_blob")))
extern externref fixpoint_create_blob(uint32_t memory_index, uint32_t length);

__attribute__((import_module("fixpoint"), import_name("attach_blob")))
extern void fixpoint_attach_blob(uint32_t memory_index, externref handle);

__attribute__((import_module("fixpoint"), import_name("len")))
extern uint32_t fixpoint_len(externref handle);

static externref resolve(const struct RustHandle *handle) {
    switch (handle->producer) {
    case COMBINATION:
        return combination_global;
    case CREATE_BLOB:
        return fixpoint_create_blob(handle->index, handle->entry);
    default:
        __builtin_unreachable();
    }
}

void attach_blob(uint32_t memory_index, const struct RustHandle *handle) {
    fixpoint_attach_blob(memory_index, resolve(handle));
}

uint32_t len(const struct RustHandle *handle) {
    return fixpoint_len(resolve(handle));
}

extern struct RustHandle _fixpoint_apply_inner(struct RustHandle combination_global);

static const struct RustHandle combination = {
    .producer = COMBINATION,
    .meta = 0x0440, // meta bits for Handle::Object(Object::Tree(Tree::Tree(_)))
};

__attribute__((export_name("_fixpoint_apply")))
externref fixpoint_apply(externref input) {
    combination_global = input;
    struct RustHandle output = _fixpoint_apply_inner(combination);
    return resolve(&output);
}