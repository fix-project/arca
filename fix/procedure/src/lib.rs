#![cfg_attr(target_arch = "wasm32", no_std)]
use dlmalloc::GlobalDlmalloc;
#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;
use fixutils::*;

extern crate alloc;
use alloc::vec::Vec;

const HELLO: &[u8] = b"hello";
const WORLD: &[u8] = b" world";

#[fix_entrypoint]
pub fn _fixpoint_apply(_combination: RustHandle) -> RustHandle {
    unsafe {
        let mut blob: Vec<u8> = HELLO.to_vec();
        memory_1_write(blob.as_ptr() as u32, blob.len());
        let handle = create_blob(1, blob.len());
        attach_blob(1, &handle);
        memory_1_read(blob.as_mut_ptr() as u32, len(&handle));
        blob.append(&mut WORLD.to_vec());
        memory_1_write(blob.as_ptr() as u32, blob.len());
        create_blob(1, blob.len())
    }
}
