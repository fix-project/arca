use crate::fixpoint::w2c_fix;
use crate::{descriptors, fixpoint, handles, rt, shell};
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_effect(fix: *mut w2c_fix, combination: u32) -> u32 {
    let combination = descriptors::address(combination);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_effect(fix, descriptors::address(result), combination) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_advise(
    fix: *mut w2c_fix,
    value: u32,
    key: u32,
    key_length: u32,
    payload: u32,
    payload_length: u32,
) {
    unsafe {
        fixpoint::w2c_fix_advise(
            fix,
            descriptors::address(value),
            key,
            key_length,
            payload,
            payload_length,
        )
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_eval(fix: *mut w2c_fix, value: u32) -> u32 {
    let value = descriptors::address(value);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_eval(fix, descriptors::address(result), value) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_return_cc(fix: *mut w2c_fix, request: u32) -> u32 {
    let request = descriptors::address(request);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_return_cc(fix, descriptors::address(result), request) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_call_cc(fix: *mut w2c_fix, callable: u32) -> u32 {
    let callable = descriptors::address(callable);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_call_cc(fix, descriptors::address(result), callable) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_access_blob(
    fix: *mut w2c_fix,
    handle: u32,
    offset: u32,
    destination: u32,
    length: u32,
) -> u32 {
    unsafe {
        fixpoint::w2c_fix_read_blob(
            fix,
            descriptors::address(handle),
            offset,
            destination,
            length,
        )
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_tree_element(
    fix: *mut w2c_fix,
    handle: u32,
    index: u32,
) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_get_tree_element(fix, descriptors::address(result), handle, index) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_attach_tree_bytes(
    fix: *mut w2c_fix,
    handle: u32,
    memory_idx: u32,
) {
    unsafe { fixpoint::w2c_fix_attach_tree_bytes(fix, descriptors::address(handle), memory_idx) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_detach_blob(fix: *mut w2c_fix, memory_idx: u32, copy: u32) {
    unsafe { fixpoint::w2c_fix_detach_blob(fix, memory_idx, copy) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_tag_unchecked(fix: *mut w2c_fix, tree: u32) -> u32 {
    let tree = descriptors::address(tree);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_tag_unchecked(fix, descriptors::address(result), tree) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_blob_i64(fix: *mut w2c_fix, value: u64) -> u32 {
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_blob_i64(fix, descriptors::address(result), value) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_blob_i32(fix: *mut w2c_fix, value: u32) -> u32 {
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_blob_i32(fix, descriptors::address(result), value) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_blob(
    fix: *mut w2c_fix,
    memory_index: u32,
    length: u32,
) -> u32 {
    let result = descriptors::allocate();
    unsafe {
        fixpoint::w2c_fix_create_blob(fix, descriptors::address(result), memory_index, length)
    };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_type(fix: *mut w2c_fix, handle: u32) -> u32 {
    unsafe { fixpoint::w2c_fix_get_type(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_data_type(fix: *mut w2c_fix, handle: u32) -> u32 {
    unsafe { fixpoint::w2c_fix_get_data_type(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_thunk_type(fix: *mut w2c_fix, handle: u32) -> u32 {
    unsafe { fixpoint::w2c_fix_get_thunk_type(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_get_encode_type(fix: *mut w2c_fix, handle: u32) -> u32 {
    unsafe { fixpoint::w2c_fix_get_encode_type(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_blob_obj(fix: *mut w2c_fix, handle: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_is_blob_obj(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_object(fix: *mut w2c_fix, handle: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_is_object(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_data(fix: *mut w2c_fix, handle: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_is_data(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_tag(fix: *mut w2c_fix, handle: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_is_tag(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_comparable(fix: *mut w2c_fix, handle: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_is_comparable(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_is_value(fix: *mut w2c_fix, handle: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_is_value(fix, descriptors::address(handle)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_unsound_handle_bytes(fix: *mut w2c_fix, handle: u32) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_unsound_handle_bytes(fix, descriptors::address(result), handle) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_read_handle_bytes(
    fix: *mut w2c_fix,
    handle: u32,
    destination: u32,
) {
    unsafe { fixpoint::w2c_fix_read_handle_bytes(fix, descriptors::address(handle), destination) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_same_handle_bytes(fix: *mut w2c_fix, lhs: u32, rhs: u32) -> u32 {
    unsafe {
        fixpoint::w2c_fix_same_handle_bytes(
            fix,
            descriptors::address(lhs),
            descriptors::address(rhs),
        )
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_equals(fix: *mut w2c_fix, lhs: u32, rhs: u32) -> i32 {
    unsafe { fixpoint::w2c_fix_equals(fix, descriptors::address(lhs), descriptors::address(rhs)) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_ref(fix: *mut w2c_fix, handle: u32) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_ref(fix, descriptors::address(result), handle) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_identification_thunk(
    fix: *mut w2c_fix,
    handle: u32,
) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe {
        fixpoint::w2c_fix_create_identification_thunk(fix, descriptors::address(result), handle)
    };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_application_thunk(fix: *mut w2c_fix, handle: u32) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe {
        fixpoint::w2c_fix_create_application_thunk(fix, descriptors::address(result), handle)
    };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_selection_thunk(fix: *mut w2c_fix, handle: u32) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_selection_thunk(fix, descriptors::address(result), handle) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_strict_encode(fix: *mut w2c_fix, handle: u32) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_strict_encode(fix, descriptors::address(result), handle) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_shallow_encode(fix: *mut w2c_fix, handle: u32) -> u32 {
    let handle = descriptors::address(handle);
    let result = descriptors::allocate();
    unsafe { fixpoint::w2c_fix_create_shallow_encode(fix, descriptors::address(result), handle) };
    result
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_len(fix: *mut w2c_fix, handle: u32) -> u64 {
    unsafe { fixpoint::w2c_fix_len(fix, descriptors::address(handle)) }
}

#[unsafe(no_mangle)]
pub extern "C" fn w2c_fix_close(_: *mut w2c_fix, index: u32) {
    descriptors::close(index);
}

#[unsafe(no_mangle)]
pub extern "C" fn w2c_fix_clone(_: *mut w2c_fix, index: u32) -> u32 {
    descriptors::insert(descriptors::address(index))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_attach_blob(fix: *mut w2c_fix, memory: u32, index: u32) -> u32 {
    unsafe { fixpoint::w2c_fix_attach_blob(fix, descriptors::address(index), memory) };
    unsafe { shell::fixpoint_len(descriptors::address(index)) as u32 }
}

fn memory_range(offset: u32, length: usize, writable: bool) -> usize {
    unsafe {
        let memory = rt::MEMORIES[0];
        assert!(!memory.is_null(), "memory 0 does not exist");
        assert!(offset as u64 + length as u64 <= (*memory).size);
        assert!(!writable || !rt::MEMORY_ATTACHED[0]);
        (*memory).data as usize + offset as usize
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_tree(_: *mut w2c_fix, pointer: u32, length: u32) -> u32 {
    let source = memory_range(pointer, length as usize * 4, false);
    let bytes = length as usize * fixhandle::HANDLE_SIZE;
    assert!(bytes <= rt::PAYLOAD_SIZE);
    let address = crate::locals::region();
    unsafe {
        rt::map_region(
            address as *mut u8,
            bytes.next_multiple_of(4096),
            arcane::__MODE_read_write,
        )
        .expect("tree allocation failed");
        for index in 0..length as usize {
            let descriptor = core::ptr::read_unaligned((source + index * 4) as *const u32);
            handles::copy_handle(
                descriptors::address(descriptor) as usize,
                address + index * 32,
                0,
            );
        }
        let result = descriptors::allocate();
        shell::fixpoint_create_tree(
            descriptors::address(result),
            core::slice::from_raw_parts(address as *const u8, bytes),
        );
        result
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_access_tree(
    fix: *mut w2c_fix,
    handle: u32,
    offset: u32,
    pointer: u32,
    length: u32,
) -> u32 {
    let value = descriptors::address(handle);
    unsafe {
        assert!(shell::fixpoint_is_object(value) && shell::fixpoint_is_tree(value));
        let size = shell::fixpoint_len(value);
        assert!(offset as usize <= size);
        let count = (length as usize).min(size - offset as usize);
        let target = memory_range(pointer, count * 4, true);
        for index in 0..count {
            let result = w2c_fix_get_tree_element(fix, handle, offset + index as u32);
            core::ptr::write_unaligned((target + index * 4) as *mut u32, result);
        }
        count as u32
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_create_tag(_: *mut w2c_fix, handle: u32) -> u32 {
    unsafe {
        let value = descriptors::address(handle);
        assert!(shell::fixpoint_is_object(value) && shell::fixpoint_is_tree(value));
        let result = descriptors::allocate();
        shell::fixpoint_create_tag(descriptors::address(result), value);
        result
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn w2c_fix_coordinate(
    _: *mut w2c_fix,
    name: u32,
    name_length: u32,
    arguments: u32,
    status: u32,
) -> u32 {
    let name_address = memory_range(name, name_length as usize, false);
    let status_address = memory_range(status, 8, true);
    let name =
        unsafe { core::slice::from_raw_parts(name_address as *const u8, name_length as usize) }
            .to_vec();
    unsafe {
        let result = descriptors::allocate();
        let reply = crate::continuation::coordinate(
            descriptors::address(result),
            &name,
            descriptors::address(arguments),
        );
        rt::copy_guest_bytes(reply.to_le_bytes().as_ptr() as usize, status_address, 8);
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn w2c_fix_exit(_: *mut w2c_fix, value: u32) -> ! {
    crate::continuation::finish(descriptors::address(value))
}
