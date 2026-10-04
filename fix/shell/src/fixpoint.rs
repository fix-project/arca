#![allow(
    improper_ctypes_definitions,
    reason = "userspace uses the System V SIMD ABI; vector layout is asserted in rt"
)]
use core::ffi::c_void;

use crate::rt::{
    EXTERNREF_SIZE, MAX_PAGES, MAX_TABLE_ELEMENTS, NUM_MEMORIES, PAGE_SIZE, SLOT_SIZE,
    wasm_rt_externref_t, wasm_rt_externref_table_t, wasm_rt_memory_t,
};
use crate::shell;

#[repr(C)]
pub struct w2c_fix(());

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_attach_blob(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
    memory_idx: u32,
) {
    assert!((memory_idx as usize) < NUM_MEMORIES);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_idx as usize];
        assert!(!memory.is_null(), "memory does not exist");
        let addr = SLOT_SIZE * memory_idx as usize;
        let len =
            shell::fixpoint_attach_blob(handle.bytes, addr as *mut c_void, (*memory).size as usize);
        (*memory).pages = len.div_ceil(PAGE_SIZE as usize) as u64;
        (*memory).max_pages = MAX_PAGES;
        (*memory).size = (*memory).pages * PAGE_SIZE as u64;
        crate::rt::MEMORY_ATTACHED[memory_idx as usize] = true;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_attach_tree(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
    table_idx: u32,
) {
    unsafe {
        let pool = crate::rt::externref_pool(table_idx);
        let table = crate::rt::TABLES[pool];
        assert!(!table.is_null(), "table does not exist");
        let addr = (*table).data as usize;
        let len = shell::fixpoint_attach_tree(
            handle.bytes,
            addr as *mut c_void,
            usize::try_from((*table).memory_size).unwrap(),
        );
        (*table).size = len as u32;
        (*table).memory_size = (len * EXTERNREF_SIZE).next_multiple_of(PAGE_SIZE as usize) as i64;
        (*table).max_size = MAX_TABLE_ELEMENTS;
        crate::rt::TABLE_ATTACHED[pool] = true;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_detach_blob(_fixpoint: *mut w2c_fix, memory_idx: u32, copy: u32) {
    assert!((memory_idx as usize) < NUM_MEMORIES);
    assert!(copy <= 1);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_idx as usize];
        assert!(!memory.is_null(), "memory does not exist");
        if copy == 0 || crate::rt::MEMORY_ATTACHED[memory_idx as usize] {
            shell::detach_data((*memory).data, (*memory).size as usize, copy != 0);
        }
        crate::rt::MEMORY_ATTACHED[memory_idx as usize] = false;
        if copy == 0 {
            (*memory).pages = 0;
            (*memory).size = 0;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_detach_tree(_fixpoint: *mut w2c_fix, table_idx: u32, copy: u32) {
    assert!(copy <= 1);
    unsafe {
        let pool = crate::rt::externref_pool(table_idx);
        let table = crate::rt::TABLES[pool];
        assert!(!table.is_null(), "table does not exist");
        if copy == 0 || crate::rt::TABLE_ATTACHED[pool] {
            shell::detach_data(
                (*table).data.cast(),
                usize::try_from((*table).memory_size).unwrap(),
                copy != 0,
            );
        }
        crate::rt::TABLE_ATTACHED[pool] = false;
        if copy == 0 {
            (*table).size = 0;
            (*table).memory_size = 0;
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_tree(
    fixpoint: *mut w2c_fix,
    table_idx: u32,
    length: u32,
) -> wasm_rt_externref_t {
    unsafe {
        let pool = crate::rt::externref_pool(table_idx);
        let table = crate::rt::TABLES[pool];
        assert!(!table.is_null(), "table does not exist");
        assert!(length <= (*table).size);
        assert_eq!(
            (*table).data as usize % core::mem::align_of::<wasm_rt_externref_t>(),
            0
        );
        wasm_rt_externref_t {
            bytes: shell::fixpoint_create_tree(core::slice::from_raw_parts(
                (*table).data.cast::<u8>(),
                length as usize * EXTERNREF_SIZE,
            )),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_tag(
    fixpoint: *mut w2c_fix,
    table_idx: u32,
) -> wasm_rt_externref_t {
    unsafe {
        let pool = crate::rt::externref_pool(table_idx);
        let table = crate::rt::TABLES[pool];
        assert!(!table.is_null(), "table does not exist");
        assert_eq!(
            (*table).data as usize % core::mem::align_of::<wasm_rt_externref_t>(),
            0
        );
        wasm_rt_externref_t {
            bytes: shell::fixpoint_create_tag(core::slice::from_raw_parts(
                (*table).data.cast::<u8>(),
                (*table).size as usize * EXTERNREF_SIZE,
            )),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_blob_i64(
    fixpoint: *mut w2c_fix,
    value: u64,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { unsafe { shell::fixpoint_create_blob_i64(value) } },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_blob_i32(
    fixpoint: *mut w2c_fix,
    value: u32,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { unsafe { shell::fixpoint_create_blob_i32(value) } },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_blob(
    fixpoint: *mut w2c_fix,
    memory_index: u32,
    length: u32,
) -> wasm_rt_externref_t {
    assert!((memory_index as usize) < NUM_MEMORIES);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_index as usize];
        assert!(!memory.is_null(), "memory does not exist");
        assert!(length as u64 <= (*memory).size);
        wasm_rt_externref_t {
            bytes: shell::fixpoint_create_blob_from_memory((*memory).data, length as usize),
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_type(
    _fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> u32 {
    (unsafe { shell::fixpoint_get_type(handle.bytes) }) as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_data_type(
    _fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> u32 {
    (unsafe { shell::fixpoint_get_data_type(handle.bytes) }) as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_encode_type(
    _fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> u32 {
    (unsafe { shell::fixpoint_get_encode_type(handle.bytes) }) as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_blob_obj(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> i32 {
    (unsafe { shell::fixpoint_is_blob_obj(handle.bytes) }) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_object(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> i32 {
    (unsafe { shell::fixpoint_is_object(handle.bytes) }) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_data(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> i32 {
    (unsafe { shell::fixpoint_is_data(handle.bytes) }) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_tag(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> i32 {
    (unsafe { shell::fixpoint_is_tag(handle.bytes) }) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_eq(fixpoint: *mut w2c_fix, handle: wasm_rt_externref_t) -> i32 {
    (unsafe { shell::fixpoint_is_eq(handle.bytes) }) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_equals(
    fixpoint: *mut w2c_fix,
    lhs: wasm_rt_externref_t,
    rhs: wasm_rt_externref_t,
) -> i32 {
    (unsafe { shell::fixpoint_equals(lhs.bytes, rhs.bytes) }) as i32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_ref(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_ref(handle.bytes) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_identification_thunk(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_identification_thunk(handle.bytes) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_application_thunk(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_application_thunk(handle.bytes) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_selection_thunk(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_selection_thunk(handle.bytes) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_strict_encode(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_strict_encode(handle.bytes) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_shallow_encode(
    fixpoint: *mut w2c_fix,
    handle: wasm_rt_externref_t,
) -> wasm_rt_externref_t {
    wasm_rt_externref_t {
        bytes: unsafe { shell::fixpoint_create_shallow_encode(handle.bytes) },
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_len(fixpoint: *mut w2c_fix, handle: wasm_rt_externref_t) -> u32 {
    u32::try_from(unsafe { shell::fixpoint_len(handle.bytes) })
        .expect("len exceeds Wasm i32 result width")
}
