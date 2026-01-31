use crate::runtime::DeterministicEquivRuntime;
use arca::Runtime as _;
use arca::{Blob, Function, Table};
use arcane::{
    __MODE_read_only, __NR_length, __TYPE_table, arca_argument, arca_blob_create, arca_blob_read,
    arca_entry, arca_mmap, arca_table_map, arcad,
};

use core::arch::x86_64::*;
use core::ffi::c_void;
use core::simd::Simd;
use fixhandle::rawhandle::{BitPack, FixHandle, Handle};
use user::ArcaError;
use user::Ref;
use user::Runtime;
use user::error::log as arca_log;
use user::error::log_int as arca_log_int;

// FixShell top-half that only handles physical handles
#[derive(Debug, Default)]
struct FixShellPhysical;
// FixShell top-half

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct u8x32(pub [u8; 32]);

#[inline(always)]
pub fn u8x32_to_simd(v: u8x32) -> Simd<u8, 32> {
    // no field access, no destructuring
    let arr: [u8; 32] = unsafe { core::mem::transmute(v) };
    Simd::from_array(arr)
}

#[inline(always)]
pub fn simd_to_u8x32(v: Simd<u8, 32>) -> u8x32 {
    let arr = v.to_array();
    unsafe { core::mem::transmute(arr) }
}

#[inline(always)]
pub fn u8x32_as_slice(v: &u8x32) -> &[u8] {
    v.0.as_slice()
}

#[derive(Debug, Default)]
struct FixShell;

impl DeterministicEquivRuntime for FixShellPhysical {
    type BlobData = Table<Runtime>;
    type TreeData = Table<Runtime>;
    type Handle = u8x32;
    type Error = ArcaError;

    fn create_blob_i64(data: u64) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_blob_i64")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_blob_i64 failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        u8x32(buf)
    }

    fn create_blob(data: Self::BlobData) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_blob")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_blob failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        u8x32(buf)
    }

    fn create_tree(data: Self::TreeData) -> Self::Handle {
        let result: Blob<Runtime> = Function::symbolic("create_tree")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .expect("create_tree failed");
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        u8x32(buf)
    }

    fn get_blob(handle: Self::Handle) -> Result<Self::BlobData, Self::Error> {
        let result: Table<Runtime> = Function::symbolic("get_blob")
            .apply(Runtime::create_blob(unsafe { u8x32_as_slice(&handle) }))
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| ArcaError::BadType)?;
        Ok(result)
    }

    fn get_tree(handle: Self::Handle) -> Result<Self::TreeData, Self::Error> {
        let result: Table<Runtime> = Function::symbolic("get_tree")
            .apply(Runtime::create_blob(unsafe { u8x32_as_slice(&handle) }))
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| ArcaError::BadType)?;
        arca_log("Got treedata");
        Ok(result)
    }

    fn is_blob(handle: Self::Handle) -> bool {
        let handle = FixHandle::unpack(u8x32_to_simd(handle));
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
        let handle = FixHandle::unpack(u8x32_to_simd(handle));

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

    fn len(handle: Self::Handle) -> usize {
        let handle = FixHandle::unpack(u8x32_to_simd(handle));
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
    type BlobData = Table<Runtime>;
    type TreeData = Table<Runtime>;
    type Handle = u8x32;
    type Error = ArcaError;

    fn create_blob_i64(data: u64) -> Self::Handle {
        FixShellPhysical::create_blob_i64(data)
    }

    fn create_blob(data: Self::BlobData) -> Self::Handle {
        FixShellPhysical::create_blob(data)
    }

    fn create_tree(data: Self::TreeData) -> Self::Handle {
        FixShellPhysical::create_tree(data)
    }

    fn get_blob(handle: Self::Handle) -> Result<Self::BlobData, Self::Error> {
        FixShellPhysical::get_blob(handle)
    }

    fn get_tree(handle: Self::Handle) -> Result<Self::TreeData, Self::Error> {
        FixShellPhysical::get_tree(handle)
    }

    fn is_blob(handle: Self::Handle) -> bool {
        FixShellPhysical::is_blob(handle)
    }

    fn is_tree(handle: Self::Handle) -> bool {
        FixShellPhysical::is_tree(handle)
    }

    fn len(handle: Self::Handle) -> usize {
        FixShellPhysical::len(handle)
    }
}

#[unsafe(no_mangle)]
#[target_feature(enable = "avx2")]
pub extern "C" fn fixpoint_create_blob_i64(val: u64) -> u8x32 {
    FixShell::create_blob_i64(val)
}

#[unsafe(no_mangle)]
#[target_feature(enable = "avx2")]
pub extern "C" fn fixpoint_attach_blob(addr: *mut c_void, handle: u8x32) -> u64 {
    if (!FixShell::is_blob(handle)) {
        arca_log("attach_blob: handle does not refer to a BlobObject");
        panic!()
    }

    let result = FixShell::get_blob(handle);

    let Ok(blob) = result else {
        arca_log("attach_blob: failed to get BlobData");
        panic!()
    };

    let mut entry = arca_entry {
        mode: __MODE_read_only,
        data: blob.clone().into_inner().as_raw() as usize,
        datatype: __TYPE_table,
    };

    unsafe { arca_mmap(addr, &mut entry) };
    FixShell::len(handle) as u64
}

#[unsafe(no_mangle)]
#[target_feature(enable = "avx2")]
pub extern "C" fn fixpoint_attach_tree(addr: *mut c_void, handle: u8x32) -> u64 {
    if (!FixShell::is_tree(handle)) {
        arca_log("attach_tree: handle does not refer to a TreeObject");
        panic!()
    }

    let result = FixShell::get_tree(handle);

    let Ok(tree) = result else {
        arca_log("attach_tree: failed to get TreeData");
        panic!()
    };

    let mut entry = arca_entry {
        mode: __MODE_read_only,
        data: tree.clone().into_inner().into_raw() as usize,
        datatype: __TYPE_table,
    };

    unsafe { arca_mmap(addr, &mut entry) };
    FixShell::len(handle) as u64
}

#[unsafe(no_mangle)]
#[target_feature(enable = "avx")]
pub extern "C" fn arca_blob_to_handle(h: i64) -> u8x32 {
    let mut buf = [0u8; 32];
    unsafe { arca_blob_read(h, 0, buf.as_mut_ptr(), 32) };
    u8x32(buf)
}

#[unsafe(no_mangle)]
#[target_feature(enable = "avx2")]
pub extern "C" fn handle_to_arca_blob(h: u8x32) -> i64 {
    unsafe { arca_blob_create(u8x32_as_slice(&h).as_ptr(), 32) }
}
