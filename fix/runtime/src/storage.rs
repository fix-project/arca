extern crate alloc;

use super::*;
use alloc::boxed::Box;
use core::option::Option;

pub mod memory;

/// A content-based blob handle, constructed by storage canonicalization.
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
        if blob.is_canonical() {
            Ok(Self(blob.into()))
        } else {
            Err(blob)
        }
    }
}

// Compare encoded identity: unpacking can retain tag bits in RawName.meta.
impl PartialEq for CanonicalHandle {
    fn eq(&self, other: &Self) -> bool {
        self.0.pack() == other.0.pack()
    }
}

impl Eq for CanonicalHandle {}

impl core::hash::Hash for CanonicalHandle {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write(&self.0.pack());
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
