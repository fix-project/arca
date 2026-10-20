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
    NotComparable,
    InvalidComparisonBit,
    InvalidValueBit,
    InvalidSelection,
}

mod data;
pub mod memory;
pub mod repository;
pub use data::{ImmutableBytes, backing_value};

/// A content-addressed handle, constructed by storage canonicalization.
#[derive(Debug, Copy, Clone)]
pub struct CanonicalHandle(Handle);

impl CanonicalHandle {
    /// # Safety
    /// All names reachable through `handle` must be canonical or inline literals.
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
    fn add_blob_backing(&self, data: ImmutableBytes) -> Blob;
    fn add_tree(&self, data: &[Handle]) -> Tree;

    /// # Safety
    /// Every entry must encode a handle with established naming provenance.
    unsafe fn add_tree_backing(&self, data: ImmutableBytes) -> Tree;

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
        if !lhs.is_comparable() || !rhs.is_comparable() {
            return Err(StorageError::NotComparable);
        }
        Ok(self.canonicalize(lhs)? == self.canonicalize(rhs)?)
    }

    fn canonicalization_certificate(&self, handle: Handle) -> Result<Tree, StorageError> {
        let canonical = self.canonicalize(handle)?;
        Ok(self.certificate(0, handle, canonical.into()))
    }

    /// Selection operands must be Objects or Refs, with Object u64 indices.
    fn select(&self, selection: Tree) -> Result<Handle, StorageError> {
        let arguments = self.get_tree(selection)?;
        let (target, indices) = arguments
            .split_first()
            .ok_or(StorageError::InvalidSelection)?;
        let object = match target.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            _ => return Err(StorageError::InvalidSelection),
        };
        let read_index = |handle: Handle| -> Result<usize, StorageError> {
            let blob = Blob::try_from(handle).map_err(|_| StorageError::InvalidSelection)?;
            let bytes = self.get_blob(blob)?;
            let integer = bytes
                .as_ref()
                .try_into()
                .map_err(|_| StorageError::InvalidSelection)?;
            usize::try_from(u64::from_le_bytes(integer)).map_err(|_| StorageError::InvalidSelection)
        };
        let (start, end, index) = match indices {
            [index] => {
                let start = read_index(*index)?;
                (
                    start,
                    start.checked_add(1).ok_or(StorageError::InvalidSelection)?,
                    true,
                )
            }
            [start, end] => (read_index(*start)?, read_index(*end)?, false),
            _ => return Err(StorageError::InvalidSelection),
        };
        if start > end || end > target.len() {
            return Err(StorageError::InvalidSelection);
        }
        Ok(match object.view() {
            ObjectView::Tree(tree) => {
                let data = self.get_tree(*tree)?;
                if index {
                    data[start]
                } else {
                    self.add_tree(&data[start..end]).into_ref().into()
                }
            }
            ObjectView::Blob(blob) => {
                let data = self.get_blob(*blob)?;
                self.add_blob(&data[start..end]).into_ref().into()
            }
        })
    }

    /// For trusted kernel/storage issuers only.
    fn certificate(&self, kind: u64, input: Handle, result: Handle) -> Tree {
        let claim = self.add_tree(&[self.add_blob(&kind.to_le_bytes()).into(), input, result]);
        self.add_tree(&[self.add_tree(&[]).into(), claim.into_ref().into()])
            .tag_descriptor()
    }

    /// Returns the assertion's Object descriptor; its data may be unavailable.
    fn certificate_claim(&self, coupon: Tree) -> Result<Tree, StorageError> {
        let fields = self.get_tree(coupon)?;
        Ok(TreeRef::try_from(fields[1]).unwrap().object_descriptor())
    }

    fn selection_certificate(&self, selection: Tree) -> Result<Tree, StorageError> {
        Ok(self.certificate(6, selection.into(), self.select(selection)?))
    }

    fn canonicalize(&self, handle: Handle) -> Result<CanonicalHandle, StorageError>;
    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError>;
    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError>;
}

impl<S: Storage + ?Sized> Storage for alloc::sync::Arc<S> {
    fn add_blob(&self, data: &[u8]) -> Blob {
        (**self).add_blob(data)
    }
    fn add_blob_backing(&self, data: ImmutableBytes) -> Blob {
        (**self).add_blob_backing(data)
    }
    fn add_tree(&self, data: &[Handle]) -> Tree {
        (**self).add_tree(data)
    }
    unsafe fn add_tree_backing(&self, data: ImmutableBytes) -> Tree {
        unsafe { (**self).add_tree_backing(data) }
    }
    fn get_blob(&self, name: Blob) -> Result<Box<[u8]>, StorageError> {
        (**self).get_blob(name)
    }
    fn get_tree(&self, name: Tree) -> Result<Box<[Handle]>, StorageError> {
        (**self).get_tree(name)
    }
    fn get_blob_backing(&self, name: Blob) -> Result<ImmutableBytes, StorageError> {
        (**self).get_blob_backing(name)
    }
    fn get_tree_backing(&self, name: Tree) -> Result<ImmutableBytes, StorageError> {
        (**self).get_tree_backing(name)
    }
    fn canonicalize(&self, handle: Handle) -> Result<CanonicalHandle, StorageError> {
        (**self).canonicalize(handle)
    }
    fn canonicalize_blob(&self, name: Blob) -> Result<CanonicalHandle, StorageError> {
        (**self).canonicalize_blob(name)
    }
    fn canonicalize_tree(&self, name: Tree) -> Result<CanonicalHandle, StorageError> {
        (**self).canonicalize_tree(name)
    }
}
