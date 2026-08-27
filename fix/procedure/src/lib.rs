#![cfg_attr(target_arch = "wasm32", no_std, feature(asm_experimental_arch))]
use dlmalloc::GlobalDlmalloc;
#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;
use fixutils::*;

num_memories!(1);
num_tables!(1);

const HELLO: &[u8] = b"hello";

#[fix_entrypoint]
pub fn _fixpoint_apply(combination: RustHandle<'static>) -> RustHandle<'static> {
    let memory_1 = Memory::new(1).expect("expected 1 memory");
    let table_1 = Table::new(1).expect("expected 1 table");

    let num_entries = combination.len();
    table_1.attach_tree(combination);
    table_1.grow(1);

    memory_1.write(HELLO);
    let blob = memory_1.create_blob(HELLO.len());
    table_1.set(num_entries, blob);

    create_strict_encode(create_identification_thunk(
        table_1.create_tree(num_entries + 1),
    ))
}
