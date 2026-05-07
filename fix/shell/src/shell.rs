use crate::runtime::DeterministicEquivRuntime;
use crate::_PROCEDURE;
use arca::{Word, Blob, Function, Table};
use arca::{Runtime as _, Tuple};
use arcane::{
    __MODE_read_only, __MODE_read_write, __NR_length, __TYPE_table, arca_argument,
    arca_blob_create, arca_blob_read, arca_compat_mmap, arca_entry, arca_mmap, arca_table_map,
    arcad,
};

use core::arch::x86_64::*;
use core::ffi::c_void;
use fixhandle::rawhandle::{BitPack, FixHandle, Handle, TreeName, Object, Thunk, Encode};
use user::ArcaError;
use user::Ref;
use user::Runtime;
use user::error::log as arca_log;
use user::error::log_int as arca_log_int;

// FixShell top-half that only handles physical handles
#[derive(Debug, Default)]
struct FixShellPhysical;
// FixShell top-half

#[derive(Debug, Default)]
pub struct FixShell;

impl DeterministicEquivRuntime for FixShellPhysical {
    type BlobData = Blob<Runtime>;
    type TreeData = Tuple<Runtime>;
    type Handle = [u8; 32];
    type Error = ArcaError;

    fn create_blob_i32(data: u32) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_blob_i32")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_blob_i32 failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }

    fn create_blob_i64(data: u64) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_blob_i64")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_blob_i64 failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }

    fn create_blob(data: Self::BlobData) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_blob")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_blob failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }

    fn create_tree(data: Self::TreeData) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_tree")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_tree failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }

    fn create_tag(data: Self::TreeData) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_tree")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_tree failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        buf
    }

    fn get_blob(handle: Self::Handle) -> Result<Self::BlobData, Self::Error> {
        let result: Blob<Runtime> = Function::symbolic("get_blob")
            .apply(Runtime::create_blob(&handle))
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| ArcaError::BadType)?;
        Ok(result)
    }

    fn get_tree(handle: Self::Handle) -> Result<Self::TreeData, Self::Error> {
        let result: Tuple<Runtime> = Function::symbolic("get_tree")
            .apply(Runtime::create_blob(&handle))
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| ArcaError::BadType)?;
        Ok(result)
    }

    fn is_equal(lhs: Self::Handle, rhs: Self::Handle) -> bool {
        let result: Word<Runtime> = Function::symbolic("is_equal")
            .apply(Runtime::create_blob(&lhs))
            .apply(Runtime::create_blob(&rhs))
            .call_with_current_continuation()
            .try_into().expect("is_equal: return type is not a word");
        
        let result = result.read();
        result == 1
    }

    fn is_blob_obj(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle);
        handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_blob_name_ref().map_err(|_| ArcaError::BadType))
            .is_ok()
    }

    fn is_blob(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle);
        handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_blob_name_ref().map_err(|_| ArcaError::BadType))
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .map_err(|_| ArcaError::BadType)
                .and_then(|h| h.try_unwrap_blob_name_ref().map_err(|_| ArcaError::BadType))
                .is_ok()
    }

    fn is_tree(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle);

        handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_tree_name_ref().map_err(|_| ArcaError::BadType))
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .map_err(|_| ArcaError::BadType)
                .and_then(|h| h.try_unwrap_tree_name_ref().map_err(|_| ArcaError::BadType))
                .is_ok()
    }

    fn is_object(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle);
        handle
            .try_unwrap_object_ref()
            .is_ok()
    }

    fn is_data(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle);
        handle
            .try_unwrap_object_ref()
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .is_ok()
    }

    fn is_tag(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle);

        let res = handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_tree_name_ref().map_err(|_| ArcaError::BadType))
            .map( |h| match h {
                TreeName::Tag(_) => true,
                TreeName::NotTag(_) => false
            }
            );

        res.unwrap_or_default()
    }

    fn create_application_thunk(handle: Self::Handle) -> Self::Handle {
        let handle = FixHandle::unpack(handle);

        let result = handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_tree_name_ref().map_err(|_| ArcaError::BadType))
            .or_else(|_| handle
                .try_unwrap_ref_ref()
                .map_err(|_| ArcaError::BadType)
                .and_then(|h| h.try_unwrap_tree_name_ref().map_err(|_| ArcaError::BadType)));

        if let Ok(tree) = result {
           FixHandle::Thunk(Thunk::Application(*tree)).pack()
        } else {
            arca_log ("create_application_thunk: input handle is not a TreeObj or TreeRef");
            panic!()
        }
    }

    fn create_strict_encode(handle: Self::Handle) -> Self::Handle {
        let handle = FixHandle::unpack(handle);

        let result = handle.try_unwrap_thunk();

        if let Ok(thunk) = result {
            FixHandle::Encode(Encode::Strict(thunk)).pack()
        } else {
            arca_log ("create_strict_encode: input handle is not a Thunk");
            panic!()
        }
    }

    fn len(handle: Self::Handle) -> usize {
        let handle = FixHandle::unpack(handle);
        let len = handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .map(|h| {
                let h: &Handle = match h {
                    fixhandle::rawhandle::Object::BlobName(blob_name) => {
                        blob_name.unwrap_blob_ref()
                    }
                    fixhandle::rawhandle::Object::TreeName(tree_name) => match tree_name {
                        fixhandle::rawhandle::TreeName::NotTag(handle) => handle,
                        fixhandle::rawhandle::TreeName::Tag(handle) => handle,
                    },
                };
                match h {
                    Handle::VirtualHandle(virtual_handle) => virtual_handle.len(),
                    Handle::PhysicalHandle(physical_handle) => physical_handle.len(),
                }
            });
        len.expect("len: failed to get size")
    }
}

impl DeterministicEquivRuntime for FixShell {
    type BlobData = Blob<Runtime>;
    type TreeData = Tuple<Runtime>;
    type Handle = [u8; 32];
    type Error = ArcaError;

    fn create_blob_i32(data: u32) -> Self::Handle {
        FixShellPhysical::create_blob_i32(data)
    }

    fn create_blob_i64(data: u64) -> Self::Handle {
        FixShellPhysical::create_blob_i64(data)
    }

    fn create_blob(data: Self::BlobData) -> Self::Handle {
        FixShellPhysical::create_blob(data)
    }

    fn create_tree(data: Self::TreeData) -> Self::Handle {
        FixShellPhysical::create_tree(data)
    }

    fn create_tag(data: Self::TreeData) -> Self::Handle {
        FixShellPhysical::create_tag(data)
    }

    fn get_blob(handle: Self::Handle) -> Result<Self::BlobData, Self::Error> {
        FixShellPhysical::get_blob(handle)
    }

    fn get_tree(handle: Self::Handle) -> Result<Self::TreeData, Self::Error> {
        FixShellPhysical::get_tree(handle)
    }

    fn is_blob_obj(handle: Self::Handle) -> bool {
       FixShellPhysical::is_blob_obj(handle) 
    }

    fn is_blob(handle: Self::Handle) -> bool {
        FixShellPhysical::is_blob(handle)
    }

    fn is_tree(handle: Self::Handle) -> bool {
        FixShellPhysical::is_tree(handle)
    }

    fn is_object(handle: Self::Handle) -> bool {
        FixShellPhysical::is_object(handle)
    }

    fn is_equal(lhs: Self::Handle, rhs: Self::Handle) -> bool {
        FixShellPhysical::is_equal(lhs, rhs)
    }

    fn is_data(handle: Self::Handle) -> bool {
        FixShellPhysical::is_data(handle)
    }

    fn is_tag(handle: Self::Handle) -> bool {
        FixShellPhysical::is_tag(handle)
    }

    fn len(handle: Self::Handle) -> usize {
        FixShellPhysical::len(handle)
    }

    fn create_application_thunk(handle: Self::Handle) -> Self::Handle {
        FixShellPhysical::create_application_thunk(handle)
    }

    fn create_strict_encode(handle: Self::Handle) -> Self::Handle {
        FixShellPhysical::create_strict_encode(handle)
    }
}

pub fn fixpoint_create_blob_i32(val: u32) -> [u8; 32] {
    FixShell::create_blob_i32(val)
}

pub fn fixpoint_create_blob_i64(val: u64) -> [u8; 32] {
    FixShell::create_blob_i64(val)
}

/// Attaches a blob to a region of memory.  Returns the size (in bytes) of the mapped blob.
///
/// # Safety
///
/// [addr] must refer to an unused region of memory which is large enough to fit the blob; there
/// must be no Rust references pointing to this region.
pub unsafe fn fixpoint_attach_blob(addr: *mut c_void, handle: [u8; 32]) -> usize {
    if (!FixShell::is_blob(handle)) {
        arca_log("attach_blob: handle does not refer to a BlobObject");
        panic!()
    }

    let result = FixShell::get_blob(handle);

    let Ok(blob) = result else {
        arca_log("attach_blob: failed to get BlobData");
        panic!()
    };
    let len = FixShell::len(handle);

    unsafe {
        arca_compat_mmap(addr, len, __MODE_read_write);
        blob.read(0, core::slice::from_raw_parts_mut(addr as *mut u8, len));
    };
    user::error::log_int("attached memory", len as u64);
    len
}

/// Attaches a tree to a region of memory.  Returns the size (in elements) of the tree.
///
/// # Safety
///
/// [addr] must refer to an unused region of memory which is large enough to fit the tree; there
/// must be no Rust references pointing to this region.  Each entry of the tree takes 32 bytes.
pub unsafe fn fixpoint_attach_tree(addr: *mut c_void, handle: [u8; 32]) -> usize {
    if (!FixShell::is_tree(handle)) {
        arca_log("attach_tree: handle does not refer to a TreeObject");
        panic!()
    }

    let result = FixShell::get_tree(handle);

    let Ok(tree) = result else {
        arca_log("attach_tree: failed to get TreeData");
        panic!()
    };

    let len = FixShell::len(handle);
    user::error::log_int("attached tree", len as u64);

    unsafe {
        arca_compat_mmap(addr, len * 32, __MODE_read_write);
        let slice = core::slice::from_raw_parts_mut(addr as *mut u8, len * 32);
        for i in 0..len {
            let element: Blob<Runtime> = tree.get(i).try_into().unwrap();
            element.read(0, &mut slice[i * 32..(i + 1) * 32]);
        }
    };
    len
}

pub unsafe fn fixpoint_create_tree(addr: *const c_void, len: usize) -> [u8; 32] {
    let mut scratch = Runtime::create_tuple( len );
    unsafe {
        let slice = core::slice::from_raw_parts(addr as *const u8, len * 32);
        for i in 0..len {
            let element = Runtime::create_blob( &slice [i * 32 .. (i + 1) * 32]);
            scratch.set(i,element); 
        }
    };
    FixShell::create_tree(scratch)
}

pub unsafe fn fixpoint_create_tag(addr: *const c_void, len: usize) -> [u8; 32] {
    /// Check that the author field matches with current procedure
    let author_field = unsafe {    core::slice::from_raw_parts(addr as *const u8, 32) };

    let procedure_ref = &raw mut _PROCEDURE;

    if unsafe { (& *procedure_ref).as_slice() } != author_field  {
        arca_log("create_tag: author does not match current procedure");
        panic!()
    };

    let result = unsafe { fixpoint_create_tree(addr, len) };
    let handle = FixHandle::unpack(result);

    let result: Result<Handle, ArcaError> =    handle
            .try_unwrap_object_ref()
            .map_err(|_| ArcaError::BadType)
            .and_then(|h| h.try_unwrap_tree_name_ref().map_err(|_| ArcaError::BadType))
            .map( |h| (*h).into() );

    let Ok(handle) = result else {
        arca_log("create_tag: created not a tree");
        panic!()
    };

    FixHandle::Object(Object::TreeName(TreeName::Tag(handle))).pack()
}

pub fn fixpoint_is_blob_obj(handle: [u8; 32]) -> i32 {
    FixShell::is_blob_obj(handle) as i32
}

pub fn fixpoint_is_object(handle: [u8; 32]) -> i32 {
    FixShell::is_object(handle) as i32
}

pub fn fixpoint_is_data(handle: [u8; 32]) -> i32 {
    FixShell::is_data(handle) as i32
}

pub fn fixpoint_is_tag(handle: [u8; 32]) -> i32 {
    FixShell::is_tag(handle) as i32
}

pub fn fixpoint_is_equal(lhs: [u8; 32], rhs: [u8; 32]) -> i32 {
    FixShell::is_equal(lhs, rhs) as i32
}

pub fn fixpoint_create_application_thunk(handle: [u8;32]) -> [u8;32] {
    FixShell::create_application_thunk(handle)
}

pub fn fixpoint_create_strict_encode(handle: [u8; 32]) -> [u8;32] {
    FixShell::create_strict_encode(handle)
}
