#![cfg_attr(target_arch = "wasm32", no_std, feature(asm_experimental_arch))]
use dlmalloc::GlobalDlmalloc;
#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;
use fixutils::*;

num_memories!(1);
num_tables!(2);

#[fix_entrypoint]
pub fn _fixpoint_apply(combination: RustHandle<'static>) -> Result<RustHandle<'static>, FixError> {
    let blob_handle = RustHandle::from_bytes(b"hello")?;
    let mut entries = combination.to_entries()?;
    entries.push(blob_handle);
    Ok(create_strict_encode(create_identification_thunk(
        RustHandle::from_entries(&entries)?,
    )))
}
