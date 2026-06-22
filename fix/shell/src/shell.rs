use crate::_PROCEDURE;
use arca::{Blob, Function, Table, Word};
use arca::{Runtime as _, Tuple};
use arcane::{
    __MODE_read_only, __MODE_read_write, __NR_length, __TYPE_table, arca_argument,
    arca_blob_create, arca_blob_read, arca_compat_mmap, arca_entry, arca_mmap, arca_table_map,
    arcad,
};

use core::arch::x86_64::*;
use core::ffi::c_void;
use fixhandle::*;

use user::ArcaError;
use user::Ref;
use user::Runtime;
use user::error::log as arca_log;
use user::error::log_int as arca_log_int;

pub fn fixpoint_create_blob_i32(val: u32) -> [u8; 32] {
    let bytes = val.to_le_bytes();
    unsafe { fixpoint_create_blob(&bytes) }
}

pub fn fixpoint_create_blob_i64(val: u64) -> [u8; 32] {
    let bytes = val.to_le_bytes();
    unsafe { fixpoint_create_blob(&bytes) }
}

/// Attaches a blob to a region of memory.  Returns the size (in bytes) of the mapped blob.
///
/// # Safety
///
/// [addr] must refer to an unused region of memory which is large enough to fit the blob; there
/// must be no Rust references pointing to this region.
pub unsafe fn fixpoint_attach_blob(addr: *mut c_void, handle: [u8; 32]) -> usize {
    if (!fixpoint_is_blob(handle)) {
        arca_log("attach_blob: handle does not refer to a BlobObject");
        panic!()
    }

    let result: Result<Blob<Runtime>, ArcaError> = Function::symbolic("get_blob")
        .apply(Runtime::create_blob(&handle))
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| ArcaError::BadType);

    let Ok(blob) = result else {
        arca_log("attach_blob: failed to get BlobData");
        panic!()
    };
    let len = fixpoint_len(handle);

    unsafe {
        arca_compat_mmap(addr, len, __MODE_read_write);
        blob.read(0, core::slice::from_raw_parts_mut(addr as *mut u8, len));
    };
    // user::error::log_int("attached memory", len as u64);
    len
}

/// Attaches a tree to a region of memory.  Returns the size (in elements) of the tree.
///
/// # Safety
///
/// [addr] must refer to an unused region of memory which is large enough to fit the tree; there
/// must be no Rust references pointing to this region.  Each entry of the tree takes 32 bytes.
pub unsafe fn fixpoint_attach_tree(addr: *mut c_void, handle: [u8; 32]) -> usize {
    if (!fixpoint_is_tree(handle)) {
        arca_log("attach_tree: handle does not refer to a TreeObject");
        panic!()
    }

    let result: Result<Blob<Runtime>, ArcaError> = Function::symbolic("get_tree")
        .apply(Runtime::create_blob(&handle))
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| ArcaError::BadType);

    let Ok(tree) = result else {
        arca_log("attach_tree: failed to get TreeData");
        panic!()
    };

    let len = fixpoint_len(handle);
    // user::error::log_int("attached tree", len as u64);

    unsafe {
        arca_compat_mmap(addr, len * 32, __MODE_read_write);
        let slice = core::slice::from_raw_parts_mut(addr as *mut u8, len * 32);
        tree.read(0, slice)
    };
    len
}

/// Creates a blob from a region of memory.  Returns the handle,
///
/// # Safety
///
/// [addr] must refer to an region of memory which is large enough for the specified [len];  
pub unsafe fn fixpoint_create_blob(slice: &[u8]) -> [u8; 32] {
    unsafe {
        let result: Blob<Runtime> = Function::symbolic("create_blob")
            .apply(slice)
            .call_with_current_continuation()
            .try_into()
            .expect("create_tree failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }
}

/// Creates a tree from a region of memory.  Returns the handle,
///
/// # Safety
///
/// [addr] must refer to an region of memory which is large enough for the specified [len];  
/// Each entry of the tree takes 32 bytes.
pub unsafe fn fixpoint_create_tree(slice: &[u8]) -> [u8; 32] {
    unsafe {
        let result: Blob<Runtime> = Function::symbolic("create_tree")
            .apply(slice)
            .call_with_current_continuation()
            .try_into()
            .expect("create_tree failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }
}

/// Creates a tag from a region of memory.  Returns the handle,
///
/// # Safety
///
/// [addr] must refer to an region of memory which is large enough for the specified [len];  
/// Each entry of the tree takes 32 bytes. _PROCEDURE must be initialized before first invocation
/// of this function.
pub unsafe fn fixpoint_create_tag(slice: &[u8]) -> [u8; 32] {
    /// Check that the author field matches with current procedure
    let author_field = &slice[..32];

    let procedure_ref = &raw mut _PROCEDURE;

    if unsafe { (&*procedure_ref).as_slice() } != author_field {
        arca_log("create_tag: author does not match current procedure");
        panic!()
    };

    let result = unsafe { fixpoint_create_tree(slice) };
    let handle = Handle::unpack(result);

    let result: Result<TreeName, ArcaError> = handle
        .try_unwrap_object_ref()
        .map_err(|_| ArcaError::BadType)
        .and_then(|h| h.try_unwrap_tree_ref().map_err(|_| ArcaError::BadType))
        .map(|h| match h {
            Tree::Tree(n) => *n,
            Tree::Tag(n) => *n,
        });

    let Ok(handle) = result else {
        arca_log("create_tag: created not a tree");
        panic!()
    };

    Handle::Object(Object::Tree(Tree::Tag(handle))).pack()
}

pub fn fixpoint_is_blob_obj(handle: [u8; 32]) -> bool {
    let handle = Handle::unpack(handle);
    handle
        .try_unwrap_object_ref()
        .map_err(|_| ArcaError::BadType)
        .and_then(|h| h.try_unwrap_blob_ref().map_err(|_| ArcaError::BadType))
        .is_ok()
}

pub fn fixpoint_is_blob(handle: [u8; 32]) -> bool {
    let handle = Handle::unpack(handle);
    handle
        .try_unwrap_object_ref()
        .map_err(|_| ArcaError::BadType)
        .and_then(|h| h.try_unwrap_blob_ref().map_err(|_| ArcaError::BadType))
        .is_ok()
        || handle
            .try_unwrap_ref_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_blob_ref().map_err(|_| ArcaError::BadType))
            .is_ok()
}

pub fn fixpoint_is_tree(handle: [u8; 32]) -> bool {
    let handle = Handle::unpack(handle);
    handle
        .try_unwrap_object_ref()
        .map_err(|_| ArcaError::BadType)
        .and_then(|h| h.try_unwrap_tree_ref().map_err(|_| ArcaError::BadType))
        .is_ok()
        || handle
            .try_unwrap_ref_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_tree_ref().map_err(|_| ArcaError::BadType))
            .is_ok()
}

pub fn fixpoint_is_object(handle: [u8; 32]) -> bool {
    let handle = Handle::unpack(handle);
    handle.try_unwrap_object_ref().is_ok()
}

pub fn fixpoint_is_data(handle: [u8; 32]) -> bool {
    let handle = Handle::unpack(handle);
    handle.try_unwrap_object_ref().is_ok() || handle.try_unwrap_ref_ref().is_ok()
}

pub fn fixpoint_is_tag(handle: [u8; 32]) -> bool {
    let handle = Handle::unpack(handle);

    let res = handle
        .try_unwrap_object_ref()
        .map_err(|_| ArcaError::BadType)
        .and_then(|h| h.try_unwrap_tree_ref().map_err(|_| ArcaError::BadType))
        .map(|h| match h {
            Tree::Tag(_) => true,
            Tree::Tree(_) => false,
        });

    res.unwrap_or_default()
}

pub fn fixpoint_is_equal(lhs: [u8; 32], rhs: [u8; 32]) -> bool {
    let result: Word<Runtime> = Function::symbolic("is_equal")
        .apply(Runtime::create_blob(&lhs))
        .apply(Runtime::create_blob(&rhs))
        .call_with_current_continuation()
        .try_into()
        .expect("is_equal: return type is not a word");

    let result = result.read();
    result == 1
}

pub fn fixpoint_create_application_thunk(handle: [u8; 32]) -> [u8; 32] {
    let handle = Handle::unpack(handle);
    // TODO: handle refs
    let thunk: Handle = Thunk::Application(handle.unwrap_object().unwrap_tree()).into();
    thunk.pack()
}

pub fn fixpoint_create_strict_encode(handle: [u8; 32]) -> [u8; 32] {
    let handle = Handle::unpack(handle);
    let encode: Handle = Encode::Strict(handle.unwrap_thunk()).into();
    encode.pack()
}

fn fixpoint_len(handle: [u8; 32]) -> usize {
    let handle = Handle::unpack(handle);
    handle.len()
}
