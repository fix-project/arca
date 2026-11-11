use core::ffi::c_void;

use crate::handles::Reference;
use crate::rt::{MAX_PAGES, NUM_MEMORIES, PAGE_SIZE, SLOT_SIZE, wasm_rt_memory_t};
use crate::shell;

#[repr(C)]
pub struct w2c_fix(());

pub extern "C" fn w2c_fix_effect(_: *mut w2c_fix, result: Reference, combination: Reference) {
    crate::continuation::transfer(result, "effect", combination)
}

fn memory_bytes(offset: u32, length: u32) -> alloc::vec::Vec<u8> {
    unsafe {
        let memory = crate::rt::MEMORIES[0];
        assert!(!memory.is_null(), "memory 0 does not exist");
        assert!(offset as u64 + length as u64 <= (*memory).size);
        let mut bytes = alloc::vec![0; length as usize];
        crate::rt::copy_guest_bytes(
            (*memory).data as usize + offset as usize,
            bytes.as_mut_ptr() as usize,
            bytes.len(),
        );
        bytes
    }
}

pub unsafe extern "C" fn w2c_fix_advise(
    _: *mut w2c_fix,
    value: Reference,
    key: u32,
    key_length: u32,
    payload: u32,
    payload_length: u32,
) {
    let key = memory_bytes(key, key_length);
    let payload = memory_bytes(payload, payload_length);
    assert!(core::str::from_utf8(&key).is_ok());
    crate::continuation::advise(value, &key, &payload);
}

pub extern "C" fn w2c_fix_eval(_: *mut w2c_fix, result: Reference, value: Reference) {
    crate::continuation::transfer(result, "eval", value)
}

pub extern "C" fn w2c_fix_return_cc(_: *mut w2c_fix, result: Reference, request: Reference) {
    crate::continuation::transfer(result, "return_cc", request)
}

pub extern "C" fn w2c_fix_call_cc(_: *mut w2c_fix, result: Reference, callable: Reference) {
    crate::continuation::transfer(result, "call_cc", callable)
}

pub unsafe extern "C" fn w2c_fix_read_blob(
    _: *mut w2c_fix,
    handle: Reference,
    offset: u32,
    destination: u32,
    length: u32,
) -> u32 {
    use arca::{Blob, Word};
    use user::Runtime;

    let length = unsafe {
        assert!(shell::fixpoint_is_blob_obj(handle), "expected Blob Object");
        let size = shell::fixpoint_len(handle);
        assert!(offset as usize <= size, "Blob offset out of bounds");
        let length = (length as usize).min(size - offset as usize);
        let memory = crate::rt::MEMORIES[0];
        assert!(!memory.is_null(), "memory 0 does not exist");
        assert!(
            destination as u64 + length as u64 <= (*memory).size,
            "Blob read destination out of bounds"
        );
        assert!(
            length == 0 || !crate::rt::MEMORY_ATTACHED[0],
            "Blob read destination is not writable"
        );
        length
    };
    if length == 0 {
        return 0;
    }
    if unsafe { crate::handles::query(handle, crate::handles::Query::Literal) } != 0 {
        unsafe {
            crate::rt::copy_guest_bytes(
                crate::handles::address(handle) + offset as usize,
                (*crate::rt::MEMORIES[0]).data as usize + destination as usize,
                length,
            );
        }
        return length as u32;
    }
    crate::locals::lift(&[handle]);
    let count: Word<Runtime> = crate::continuation::operation("read_blob")
        .blob(&crate::handles::bytes(handle))
        .word(offset as u64)
        .word(destination as u64)
        .word(length as u64)
        .call_with_current_continuation()
        .try_into()
        .expect("read_blob: expected byte count");
    count.read() as u32
}

pub unsafe extern "C" fn w2c_fix_attach_blob(
    fixpoint: *mut w2c_fix,
    handle: Reference,
    memory_idx: u32,
) {
    assert!((memory_idx as usize) < NUM_MEMORIES);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_idx as usize];
        assert!(!memory.is_null(), "memory does not exist");
        let addr = SLOT_SIZE * memory_idx as usize;
        let len = shell::fixpoint_attach_blob(handle, addr as *mut c_void, (*memory).size as usize);
        (*memory).pages = len.div_ceil(PAGE_SIZE as usize) as u64;
        (*memory).max_pages = MAX_PAGES;
        (*memory).size = (*memory).pages * PAGE_SIZE as u64;
        crate::rt::MEMORY_ATTACHED[memory_idx as usize] = true;
        crate::rt::retain_memory(memory_idx as usize, handle);
    }
}

pub extern "C" fn w2c_fix_get_tree_element(
    _: *mut w2c_fix,
    result: Reference,
    handle: Reference,
    index: u32,
) {
    use arca::{Blob, Word};
    use user::Runtime;

    if !unsafe { shell::fixpoint_is_object(handle) && shell::fixpoint_is_tree(handle) } {
        crate::continuation::trap(crate::rt::wasm_rt_trap_t_WASM_RT_TRAP_UNREACHABLE);
    }
    if index as usize >= unsafe { shell::fixpoint_len(handle) } {
        crate::continuation::trap(crate::rt::wasm_rt_trap_t_WASM_RT_TRAP_OOB);
    }
    crate::locals::lift(&[handle]);
    crate::continuation::receive(
        result,
        crate::continuation::operation("get_tree_element")
            .blob(&crate::handles::bytes(handle))
            .word(index as u64),
    )
}

pub unsafe extern "C" fn w2c_fix_attach_tree_bytes(
    _: *mut w2c_fix,
    handle: Reference,
    memory_idx: u32,
) {
    assert!((memory_idx as usize) < NUM_MEMORIES);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_idx as usize];
        assert!(!memory.is_null(), "memory does not exist");
        let addr = SLOT_SIZE * memory_idx as usize;
        let len =
            shell::fixpoint_attach_tree_bytes(handle, addr as *mut c_void, (*memory).size as usize);
        (*memory).pages = len.div_ceil(PAGE_SIZE as usize) as u64;
        (*memory).max_pages = MAX_PAGES;
        (*memory).size = (*memory).pages * PAGE_SIZE as u64;
        crate::rt::MEMORY_ATTACHED[memory_idx as usize] = true;
        crate::rt::retain_memory(memory_idx as usize, handle);
    }
}

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
        crate::rt::release_memory(memory_idx as usize);
        if copy == 0 {
            (*memory).pages = 0;
            (*memory).size = 0;
        }
    }
}

pub extern "C" fn w2c_fix_create_tag_unchecked(
    _: *mut w2c_fix,
    result: Reference,
    tree: Reference,
) {
    use arca::{Blob, Word};
    use user::Runtime;
    let allowed: Word<Runtime> = crate::continuation::operation("is_trusted_tagger")
        .call_with_current_continuation()
        .try_into()
        .expect("expected tagger permission");
    if allowed.read() != 1 {
        crate::continuation::trap(crate::rt::wasm_rt_trap_t_WASM_RT_TRAP_UNREACHABLE);
    }
    unsafe {
        assert!(shell::fixpoint_is_object(tree) && shell::fixpoint_is_tree(tree));
        assert!(shell::fixpoint_len(tree) > 0);
    }
    crate::locals::lift(&[tree]);
    crate::continuation::receive(
        result,
        crate::continuation::operation("create_tag_unchecked").blob(&crate::handles::bytes(tree)),
    )
}

pub unsafe extern "C" fn w2c_fix_create_blob_i64(
    fixpoint: *mut w2c_fix,
    result: Reference,
    value: u64,
) {
    unsafe { shell::fixpoint_create_blob_i64(result, value) }
}

pub unsafe extern "C" fn w2c_fix_create_blob_i32(
    fixpoint: *mut w2c_fix,
    result: Reference,
    value: u32,
) {
    unsafe { shell::fixpoint_create_blob_i32(result, value) }
}

pub unsafe extern "C" fn w2c_fix_create_blob(
    fixpoint: *mut w2c_fix,
    result: Reference,
    memory_index: u32,
    length: u32,
) {
    assert!((memory_index as usize) < NUM_MEMORIES);
    unsafe {
        let memory = crate::rt::MEMORIES[memory_index as usize];
        assert!(!memory.is_null(), "memory does not exist");
        assert!(length as u64 <= (*memory).size);
        shell::fixpoint_create_blob_from_memory(result, (*memory).data, length as usize)
    }
}

pub unsafe extern "C" fn w2c_fix_get_type(_fixpoint: *mut w2c_fix, handle: Reference) -> u32 {
    (unsafe { shell::fixpoint_get_type(handle) }) as u32
}

pub unsafe extern "C" fn w2c_fix_get_data_type(_fixpoint: *mut w2c_fix, handle: Reference) -> u32 {
    (unsafe { shell::fixpoint_get_data_type(handle) }) as u32
}

pub unsafe extern "C" fn w2c_fix_get_thunk_type(_fixpoint: *mut w2c_fix, handle: Reference) -> u32 {
    (unsafe { shell::fixpoint_get_thunk_type(handle) }) as u32
}

pub unsafe extern "C" fn w2c_fix_get_encode_type(
    _fixpoint: *mut w2c_fix,
    handle: Reference,
) -> u32 {
    (unsafe { shell::fixpoint_get_encode_type(handle) }) as u32
}

pub unsafe extern "C" fn w2c_fix_is_blob_obj(fixpoint: *mut w2c_fix, handle: Reference) -> i32 {
    (unsafe { shell::fixpoint_is_blob_obj(handle) }) as i32
}

pub unsafe extern "C" fn w2c_fix_is_object(fixpoint: *mut w2c_fix, handle: Reference) -> i32 {
    (unsafe { shell::fixpoint_is_object(handle) }) as i32
}

pub unsafe extern "C" fn w2c_fix_is_data(fixpoint: *mut w2c_fix, handle: Reference) -> i32 {
    (unsafe { shell::fixpoint_is_data(handle) }) as i32
}

pub unsafe extern "C" fn w2c_fix_is_tag(fixpoint: *mut w2c_fix, handle: Reference) -> i32 {
    (unsafe { shell::fixpoint_is_tag(handle) }) as i32
}

pub unsafe extern "C" fn w2c_fix_is_comparable(fixpoint: *mut w2c_fix, handle: Reference) -> i32 {
    (unsafe { shell::fixpoint_is_comparable(handle) }) as i32
}

pub unsafe extern "C" fn w2c_fix_is_value(fixpoint: *mut w2c_fix, handle: Reference) -> i32 {
    (unsafe { shell::fixpoint_is_value(handle) }) as i32
}

pub unsafe extern "C" fn w2c_fix_unsound_handle_bytes(
    _: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_unsound_handle_bytes(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_read_handle_bytes(
    _: *mut w2c_fix,
    handle: Reference,
    destination: u32,
) {
    shell::require_privilege();
    unsafe {
        let memory = &*crate::rt::MEMORIES[0];
        assert!(!crate::rt::MEMORY_ATTACHED[0]);
        assert!(destination as u64 + 32 <= memory.size);
        crate::locals::lift(&[handle]);
        crate::handles::copy_handle(
            crate::handles::address(handle),
            memory.data as usize + destination as usize,
            0,
        );
    }
}

pub extern "C" fn w2c_fix_same_handle_bytes(
    _: *mut w2c_fix,
    lhs: Reference,
    rhs: Reference,
) -> u32 {
    shell::require_privilege();
    crate::locals::lift(&[lhs, rhs]);
    unsafe {
        crate::handles::same_handle_bytes(
            crate::handles::address(lhs),
            crate::handles::address(rhs),
        )
    }
}

pub unsafe extern "C" fn w2c_fix_equals(
    fixpoint: *mut w2c_fix,
    lhs: Reference,
    rhs: Reference,
) -> i32 {
    (unsafe { shell::fixpoint_equals(lhs, rhs) }) as i32
}

pub unsafe extern "C" fn w2c_fix_create_ref(
    fixpoint: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_create_ref(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_create_identification_thunk(
    fixpoint: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_create_identification_thunk(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_create_application_thunk(
    fixpoint: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_create_application_thunk(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_create_selection_thunk(
    fixpoint: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_create_selection_thunk(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_create_strict_encode(
    fixpoint: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_create_strict_encode(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_create_shallow_encode(
    fixpoint: *mut w2c_fix,
    result: Reference,
    handle: Reference,
) {
    unsafe { shell::fixpoint_create_shallow_encode(result, handle) }
}

pub unsafe extern "C" fn w2c_fix_len(fixpoint: *mut w2c_fix, handle: Reference) -> u64 {
    unsafe { shell::fixpoint_len(handle) as u64 }
}
