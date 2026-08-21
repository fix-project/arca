#ifndef FIX_UTILS
#define FIX_UTILS
#include <stdint.h>

typedef __externref_t externref;

enum producer {
    COMBINATION = 0,
    TABLE_GET = 1,
    CREATE_BLOB = 2,
    CREATE_TREE = 3,
};

enum handle {
    REF = 0,
    OBJECT = 1,
    THUNK = 2,
    ENCODE = 3
};
enum thunk {
    IDENTIFICATION = 0,
    APPLICATION = 1,
    SELECTION = 2
};
enum encode {
    STRICT = 0,
    SHALLOW = 1
};

#define PRODUCER_TAG(meta) (((meta) >> 12) & 0x3)
#define HANDLE_TAG(meta) (((meta) >> 10) & 0x3)
#define ENCODE_TAG(meta) (((meta) >> 9) & 0x1)
#define THUNK_TAG(meta) (((meta) >> 7) & 0x3)

struct RustHandle {
    uint8_t name[24];
    union {
        uint64_t body;
        struct {
            uint32_t entry;
            uint16_t index;
            uint16_t meta;
        };
    };
};

__attribute__((import_module("fixpoint"), import_name("create_blob")))
extern externref fixpoint_create_blob(uint32_t memory_index, uint32_t length);

__attribute__((import_module("fixpoint"), import_name("create_tree")))
extern externref fixpoint_create_tree(uint32_t table_index, uint32_t length);

__attribute__((import_module("fixpoint"), import_name("create_ref"))) 
extern externref fixpoint_create_ref(externref handle);

__attribute__((import_module("fixpoint"), import_name("create_identification_thunk")))
extern externref fixpoint_create_identification_thunk(externref handle);

__attribute__((import_module("fixpoint"), import_name("create_application_thunk")))
extern externref fixpoint_create_application_thunk(externref handle);

__attribute__((import_module("fixpoint"), import_name("create_selection_thunk")))
extern externref fixpoint_create_selection_thunk(externref handle);

__attribute__((import_module("fixpoint"), import_name("create_strict_encode")))
extern externref fixpoint_create_strict_encode(externref handle);

__attribute__((import_module("fixpoint"), import_name("create_shallow_encode")))
extern externref fixpoint_create_shallow_encode(externref handle);

__attribute__((import_module("fixpoint"), import_name("attach_blob")))
extern void fixpoint_attach_blob(uint32_t memory_index, externref handle);

__attribute__((import_module("fixpoint"), import_name("attach_tree")))
extern void fixpoint_attach_tree(uint32_t table_index, externref handle);

__attribute__((import_module("fixpoint"), import_name("len")))
extern uint32_t fixpoint_len(externref handle);

extern externref fixpoint_table_get(uint32_t table_index, uint32_t entry_index);
extern void fixpoint_table_set(uint32_t table_index, uint32_t entry_index, externref value);
extern struct RustHandle _fixpoint_apply_inner(struct RustHandle combination);

#endif