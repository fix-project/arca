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
        (*memory).pages = len.div_ceil(PAGE_SIZE as usize) as u64;
        (*memory).max_pages = (1u64 << 32) / PAGE_SIZE as u64;
        (*memory).size = (*memory).pages * PAGE_SIZE as u64;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_attach_tree(
    fixpoint: *mut w2c_fixpoint,
    table_idx: u32,
    handle: wasm_rt_externref_t,
) {
    assert!(table_idx < 32);
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
pub unsafe extern "C" fn w2c_fixpoint_create_tree(
    fixpoint: *mut w2c_fixpoint,
    table_idx: u32,
    length: u32,
) -> wasm_rt_externref_t {
    assert!(table_idx < 32);
    unsafe {
        let table = crate::rt::TABLES[table_idx as usize];
        wasm_rt_externref_t {
            bytes: shell::fixpoint_create_tree(core::slice::from_raw_parts(
                (*table).data.cast::<u8>(),
                length as usize * 32,
            )),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_tag(
    fixpoint: *mut w2c_fixpoint,
    table_idx: u32,
    length: u32,
) -> wasm_rt_externref_t {
    assert!(table_idx < 32);
    unsafe {
        let table = crate::rt::TABLES[table_idx as usize];
        wasm_rt_externref_t {
            bytes: shell::fixpoint_create_tag(core::slice::from_raw_parts(
                (*table).data.cast::<u8>(),
                length as usize * 32,
            )),
        }
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_blob_i32(
    fixpoint: *mut w2c_fixpoint,
    value: u32,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_blob_i32(value) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_blob(
    fixpoint: *mut w2c_fixpoint,
    memory_index: u32,
    length: u32,
) -> wasm_rt_externref_t {
    assert!(memory_index < 63);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_index as usize];
        wasm_rt_externref_t {
            bytes: shell::fixpoint_create_blob(core::slice::from_raw_parts(
                (*memory).data,
                length as usize,
            )),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_is_blob_obj(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> i32 {
    shell::fixpoint_is_blob_obj(handle.bytes) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_is_object(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> i32 {
    shell::fixpoint_is_object(handle.bytes) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_is_data(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> i32 {
    shell::fixpoint_is_data(handle.bytes) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_is_tag(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> i32 {
    shell::fixpoint_is_tag(handle.bytes) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_is_equal(
    fixpoint: *mut w2c_fixpoint,
    lhs: wasm_rt_externref_t,
    rhs: wasm_rt_externref_t,
) -> i32 {
    shell::fixpoint_is_equal(lhs.bytes, rhs.bytes) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_ref(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: shell::fixpoint_create_ref(handle.bytes),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_identification_thunk(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: shell::fixpoint_create_identification_thunk(handle.bytes),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_application_thunk(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: shell::fixpoint_create_application_thunk(handle.bytes),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_selection_thunk(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: shell::fixpoint_create_selection_thunk(handle.bytes),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_strict_encode(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: shell::fixpoint_create_strict_encode(handle.bytes),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_create_shallow_encode(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: shell::fixpoint_create_shallow_encode(handle.bytes),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fixpoint_len(
    fixpoint: *mut w2c_fixpoint,
    handle: wasm_rt_externref_t,
) -> usize {
    shell::fixpoint_len(handle.bytes)
}
