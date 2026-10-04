extern crate alloc;

use super::*;
use alloc::boxed::Box;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageError {
    WrongStorage,
    UnsupportedName,
    NotFound,
    LengthMismatch,
    Cycle,
    NotEq,
    InvalidEq,
}

mod data;
pub mod memory;
pub use data::{ImmutableBytes, backing_value};

/// A content-addressed handle, constructed by storage canonicalization.
#[derive(Debug, Copy, Clone)]
pub struct CanonicalHandle(Handle);

impl CanonicalHandle {
    /// # Safety
    /// `Handle` must be canonicalized.
    /// For blobs this means being an inline literal or containing a canonical name,
    /// For trees this means all children `Handle`s must be canonicalized.
    pub unsafe fn new(handle: Handle) -> Self {
        Self(handle)
    }
}

impl From<CanonicalHandle> for Handle {
    fn from(canonical: CanonicalHandle) -> Self {
        canonical.0
    }
}

impl TryFrom<Blob> for CanonicalHandle {
    type Error = Blob;

    fn try_from(blob: Blob) -> Result<Self, Self::Error> {
        if blob.as_handle().is_canonical() {
            Ok(Self(blob.into()))
        } else {
            Err(blob)
        }
    }
}

impl TryFrom<Tree> for CanonicalHandle {
    type Error = Tree;

    fn try_from(tree: Tree) -> Result<Self, Self::Error> {
        if tree.as_handle().is_canonical() {
            Ok(Self(tree.into()))
        } else {
            Err(tree)
        }
    }
}

impl PartialEq for CanonicalHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_bytes() == other.0.as_bytes()
    }
}

impl Eq for CanonicalHandle {}

impl core::hash::Hash for CanonicalHandle {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write(self.0.as_bytes());
    }
}

/// An object store, capable of saving and retrieving Fix objects.
pub trait Storage {
    fn add_blob(&self, data: &[u8]) -> Blob;
    fn add_tree(&self, data: &[Handle]) -> Tree;

    fn get_blob(&self, name: Blob) -> Result<Box<[u8]>, StorageError>;
    fn get_tree(&self, name: Tree) -> Result<Box<[Handle]>, StorageError>;
    fn get_blob_backing(&self, name: Blob) -> Result<ImmutableBytes, StorageError>;
    fn get_tree_backing(&self, name: Tree) -> Result<ImmutableBytes, StorageError>;

    fn has_blob(&self, name: Blob) -> bool {
        self.get_blob(name).is_ok()
    }

    fn has_tree(&self, name: Tree) -> bool {
        self.get_tree(name).is_ok()
    }

    fn equals(&self, lhs: Handle, rhs: Handle) -> Result<bool, StorageError> {
        fn canonicalize(
            storage: &(impl Storage + ?Sized),
            handle: Handle,
        ) -> Result<Handle, StorageError> {
            let (object, reference) = match handle.view() {
                HandleView::Object(object) => (*object, false),
                HandleView::Ref(reference) => (reference.object_descriptor(), true),
                _ => return Err(StorageError::NotEq),
            };
            let canonical: Handle = match object.view() {
                ObjectView::Blob(blob) => storage.canonicalize_blob(*blob)?,
                ObjectView::Tree(tree) => storage.canonicalize_tree(*tree)?,
            }
            .into();
            Ok(if reference {
                Object::try_from(canonical).unwrap().into_ref().into()
            } else {
                canonical
            })
        }
        if !lhs.is_eq() || !rhs.is_eq() {
            return Err(StorageError::NotEq);
        }
        let lhs = canonicalize(self, lhs)?;
        let rhs = canonicalize(self, rhs)?;
        Ok(lhs.same_encoding(&rhs))
    }

    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError>;
    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError>;
}
