#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use core::simd::u8x32;

use crate::{
    data::{BlobData, TreeData},
    runtime::{DeterministicEquivRuntime, ExecutionRuntime},
    storage::{ObjectStore, Storage},
};
use arca::Runtime;
use bytemuck::bytes_of;
use derive_more::TryUnwrapError;
use fixhandle::rawhandle::{BitPack, FixHandle, Object};
use kernel::types::Blob as ArcaBlob;
use kernel::{prelude::vec, types::Value};

#[derive(Debug)]
pub enum Error {
    OOB,
    TypeMismatch,
}

impl<T> From<TryUnwrapError<T>> for Error {
    fn from(_value: TryUnwrapError<T>) -> Self {
        Error::TypeMismatch
    }
}

#[derive(Default, Debug)]
pub struct FixRuntime {
    store: ObjectStore,
}

impl FixRuntime {
    fn new() -> Self {
        Self::default()
    }
}

impl DeterministicEquivRuntime for FixRuntime {
    type BlobData = BlobData;
    type TreeData = TreeData;
    type Handle = FixHandle;
    type Error = Error;

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        let buf = bytes_of(&data);
        Object::from(self.store.create_blob(BlobData::create(buf))).into()
    }

    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle {
        Object::from(self.store.create_blob(data)).into()
    }

    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle {
        Object::from(self.store.create_tree(data)).into()
    }

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error> {
        let b = handle
            .try_unwrap_object_ref()
            .map_err(Error::from)?
            .try_unwrap_blob_name_ref()
            .map_err(|_| Error::TypeMismatch)?;
        Ok(self.store.get_blob(b))
    }

    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error> {
        let t = handle
            .try_unwrap_object_ref()
            .map_err(Error::from)?
            .try_unwrap_tree_name_ref()
            .map_err(Error::from)?;
        Ok(self.store.get_tree(t))
    }

    fn is_blob(handle: &Self::Handle) -> bool {
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

fn pack_handle(handle: &FixHandle) -> ArcaBlob {
    let raw = handle.pack();
    Runtime::create_blob(raw.as_array())
}

fn unpack_handle(blob: ArcaBlob) -> FixHandle {
    let mut buf = [0u8; 32];
    if Runtime::read_blob(&blob, 0, &mut buf) != 32 {
        panic!("Failed to parse Arca Blob to Fix Handle")
    }
    FixHandle::unpack(u8x32::from_array(buf))
}

impl ExecutionRuntime for FixRuntime {
    fn execute(&mut self, combination: &Self::Handle) -> Result<Self::Handle, Self::Error> {
        let tree = self.get_tree(combination)?;
        let function_handle = tree.get(1);
        let elf = self.get_blob(&function_handle)?;

        let mut buffer = vec![0u8; elf.len()];
        elf.get(&mut buffer);

        let f = common::elfloader::load_elf(&buffer).expect("Failed to load elf");
        let f = Runtime::apply_function(f, Value::from(pack_handle(combination)));

        let result = f.force();
        if let Value::Blob(b) = result {
            Ok(unpack_handle(b))
        } else {
            Err(Error::TypeMismatch)
        }
    }
}
