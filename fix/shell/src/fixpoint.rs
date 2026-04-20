use core::ffi::c_void;

use crate::rt::{PAGE_SIZE, wasm_rt_externref_t, wasm_rt_externref_table_t, wasm_rt_memory_t};
use crate::shell;

#[repr(C)]
pub struct w2c_fixpoint(());

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_attach_blob(
    fixpoint: *mut w2c_fixpoint,
    memory_idx: u32,
    handle: wasm_rt_externref_t,
) {
    assert!(memory_idx < 64);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_idx as usize];
        if (memory.is_null()) {
            return;
        }
        let addr = (1usize << 32) * memory_idx as usize;
        let len = shell::fixpoint_attach_blob(addr as *mut c_void, handle.bytes);
        // TODO: this math is wrong
        (*memory).pages = (len as u64 / PAGE_SIZE as u64) + 1;
        (*memory).max_pages = (1u64 << 32) / PAGE_SIZE as u64;
        (*memory).size = len as u64;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_attach_tree(
    fixpoint: *mut w2c_fixpoint,
    table_idx: u32,
    handle: wasm_rt_externref_t,
) {
    assert!(table_idx < 63);
    unsafe {
        let table = crate::rt::TABLES[table_idx as usize];
        if (table.is_null()) {
            return;
        }
        let addr = (1usize << 32) * (64 + table_idx as usize);
        let len = shell::fixpoint_attach_tree(addr as *mut c_void, handle.bytes);
        (*table).size = len as u32;
        (*table).max_size = (1 << (32 - 5)) as u32;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_blob_i64(
    fixpoint: *mut w2c_fixpoint,
    value: u64,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_blob_i64(value) },
    }
}
