#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::{
    data::{BlobData, TreeData},
    runtime::DeterministicEquivRuntime,
    storage::{ObjectStore, Storage},
};
use bytemuck::bytes_of;
use derive_more::TryUnwrapError;
use fixhandle::rawhandle::{FixHandle, Object};

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

#[derive(Debug)]
pub struct FixRuntime<'a> {
    store: &'a mut ObjectStore,
}

impl<'a> FixRuntime<'a> {
    fn new(store: &'a mut ObjectStore) -> Self {
        Self { store }
    }
}

impl<'a> DeterministicEquivRuntime for FixRuntime<'a> {
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
