extern crate alloc;

use super::*;
use alloc::boxed::Box;
use core::option::Option;

pub mod memory;

/// A content-based blob handle, constructed by storage canonicalization.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct CanonicalHandle(Handle);

impl From<CanonicalHandle> for Handle {
    fn from(canonical: CanonicalHandle) -> Self {
        canonical.0
    }
}

/// An object store, capable of saving and retrieving Fix objects.
pub trait Storage {
    fn add_blob(&self, data: &[u8]) -> Blob;
    fn add_tree(&self, data: &[Handle]) -> Tree;

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>>;
    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>>;

    fn has_blob(&self, name: Blob) -> bool {
        self.get_blob(name).is_some()
    }

    fn has_tree(&self, name: Tree) -> bool {
        self.get_tree(name).is_some()
    }

    fn canonicalize_blob(&self, blob: Blob) -> Option<CanonicalHandle>;
    fn canonicalize_tree(&self, tree: Tree) -> Option<CanonicalHandle>;
}

/// An object store that is capable of saving and retrieving canonically named Fix objects.
pub trait CanonicalStorage {
        /// Name blob contents without storing them. Short blobs remain inline literals.
    ///
    /// # Panics
    /// Panics if the length exceeds the handle's 48-bit size field.
    fn canonicalize_blob(&self, bytes: &[u8]) -> CanonicalHandle;

}
