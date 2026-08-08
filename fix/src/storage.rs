extern crate alloc;

use super::*;
use alloc::boxed::Box;
use common::protocol::control::ErrorKind;
use core::option::Option;

pub mod disk;
pub mod memory;

#[derive(Debug, Copy, Clone)]
pub struct StorageError(pub ErrorKind);

impl From<ErrorKind> for StorageError {
    fn from(value: ErrorKind) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
pub enum ImportError {
    Unresolved(Handle),
    Storage(StorageError),
}

impl From<StorageError> for ImportError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

/// An object store, capable of saving and retrieving Fix objects.
pub trait Storage {
    fn add_blob(&self, data: &[u8]) -> Result<Blob, StorageError>;
    fn add_tree(&self, data: &[Handle]) -> Result<Tree, StorageError>;

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>>;
    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>>;

    fn import(&self, from: &dyn Storage, handle: Handle) -> Result<Handle, ImportError>;

    fn has_blob(&self, name: Blob) -> bool {
        self.get_blob(name).is_some()
    }

    fn has_tree(&self, name: Tree) -> bool {
        self.get_tree(name).is_some()
    }

    fn export(&self, handle: Handle, to: &dyn Storage) -> Result<Handle, ImportError>
    where
        Self: Sized,
    {
        to.import(self, handle)
    }
}
