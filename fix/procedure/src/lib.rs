#![cfg_attr(target_arch = "wasm32", no_std)]

use dlmalloc::GlobalDlmalloc;

use externref::{Resource, externref};
use fixutils::{Handle, create_blob, create_tree, get_blob, get_tree};

#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;

use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

const CONTENT: &[u8] = b"hello";

#[externref]
#[unsafe(no_mangle)]
pub extern "C" fn _fixpoint_apply(_combination: Resource<Handle>) -> Resource<Handle> {
    // Temporary fix procedure to test and debug utils and instrumentation
    let blob = create_blob(CONTENT);
    let tree = create_tree(&[blob]);
    let mut tree = get_tree(tree);
    let blob = get_blob(tree.pop().unwrap());
    create_blob(&blob)
}
