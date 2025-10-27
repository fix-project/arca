use crate::runtime::DeterministicEquivRuntime;
use arca::{Blob, Function};
use arca::{Runtime, Table};
use arcane::{__NR_length, arca_mmap};
use core::arch::x86_64::__m256i;
use core::ffi::c_void;
use fixhandle::rawhandle::{BitPack, FixHandle, Handle};
use user::ArcaError;
use user::Ref;
use user::Runtime;
use user::error::log as arca_log;

// FixShell top-half that only handles physical handles
#[derive(Debug, Default)]
struct FixShellPhysical;
// FixShell top-half

#[derive(Debug, Default)]
struct FixShell {
    inner: FixShellPhysical,
}

struct Error;

impl DeterministicEquivRuntime for FixShellPhysical {
    type BlobData = Table;
    type TreeData = Table;
    type Handle = __m256i;
    type Error = ArcaError;

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        let result: Blob = Function::symbolic("create_blob_i64")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        __m256i::from(buf)
    }

    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle {
        let result: Blob = Function::symbolic("create_blob")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        __m256i::from(buf)
    }

    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle {
        let result: Blob = Function::symbolic("create_tree")
            .apply(data)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        let mut buf = [0u8; 32];
        Runtime::read_blob(&result, 0, &mut buf);
        __m256i::from(buf)
    }

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error> {
        let result: Table = Function::symbolic("get_blob")
            .apply(Runtime::create_blob(&__m256i.into()))
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        result
    }

    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error> {
        let result: Table = Function::symbolic("get_tree")
            .apply(Runtime::create_blob(&__m256i.into()))
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        result
    }

    fn is_blob(handle: &Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle.clone().into());
        handle
            .try_unwrap_object_ref()
            .map_err(Error::from)
            .and_then(|h| h.try_unwrap_blob_name_ref().map_err(Error::from))
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .map_err(Error::from)
                .and_then(|h| h.try_unwrap_blob_name_ref().map_err(Error::from))
                .is_ok()
    }

    fn is_tree(handle: &Self::Handle) -> bool {
        let handle = FixHandle::unpack(handle.clone().into());
        handle
            .try_unwrap_object_ref()
            .map_err(Error::from)
            .and_then(|h| h.try_unwrap_tree_name_ref().map_err(Error::from))
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .map_err(Error::from)
                .and_then(|h| h.try_unwrap_tree_name_ref().map_err(Error::from))
                .is_ok()
    }
}

impl FixShellPhysical {
    fn len(handle: &__m256i) -> usize {
        let handle = FixHandle::unpack(handle.clone().into());
        let len = handle
            .try_unwrap_object_ref()
            .map_err(Error::from)
            .and_then(|h| {
                let h: &Handle = match h {
                    fixhandle::rawhandle::Object::BlobName(blob_name) => blob_name.unwrap_blob(),
                    fixhandle::rawhandle::Object::TreeName(tree_name) => match tree_name {
                        fixhandle::rawhandle::TreeName::NotTag(handle) => handle,
                        fixhandle::rawhandle::TreeName::Tag(handle) => handle,
                    },
                };
                match h {
                    Handle::VirtualHandle(virtual_handle) => Ok(virtual_handle.len()),
                    Handle::PhysicalHandle(physical_handle) => Ok(physical_handle.len()),
                }
            });
        len.expect("len: failed to get size")
    }
}

impl DeterministicEquivRuntime for FixShell {
    type BlobData = Table;
    type TreeData = Table;
    type Handle = __m256i;
    type Error = Error;

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        self.inner.create_blob_i64(data)
    }

    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle {
        self.inner.create_blob(data)
    }

    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle {
        self.inner.create_tree(data)
    }

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error> {
        self.inner.get_blob(handle)
    }

    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error> {
        self.inner.get_tree(handle)
    }

    fn is_blob(handle: &Self::Handle) -> bool {
        FixShellPhysical::is_blob(handle)
    }

    fn is_tree(handle: &Self::Handle) -> bool {
        FixShellPhysical::is_tree(handle)
    }
}

impl FixShell {
    fn len(handle: &__m256i) -> usize {
        FixShellPhysical::len(handle)
    }
}

static FIXSHELL: FixShell = FixShell;

#[no_mangle]
pub extern "C" fn fixpoint_create_blob_i64(val: u64) -> __m256i {
    FIXSHELL.create_blob_i64(val)
}

#[no_mangle]
pub extern "C" fn fixpoint_attach_blob(addr: *mut c_void, handle: __m256i) -> u64 {
    if (!FixShell::is_blob(handle)) {
        arca_log("attach_blob: handle does not refer to a BlobObject");
        panic!()
    }

    let Ok(blob) = FIXSHELL.get_blob(handle) else {
        arca_log("attach_blob: failed to get BlobData");
        panic!()
    };

    unsafe { arca_mmap(addr, blob) };
    FixShell::len(handle)
}

#[no_mangle]
pub extern "C" fn fixpoint_attach_tree(addr: *mut c_void, handle: __m256i) -> u64 {
    if (!FixShell::is_tree(handle)) {
        arca_log("attach_tree: handle does not refer to a BlobObject");
        panic!()
    }

    let Ok(tree) = FIXSHELL.get_tree(handle) else {
        arca_log("attach_tree: failed to get BlobData");
        panic!()
    };

    unsafe { arca_mmap(addr, tree) };
    FixShell::len(handle)
}
