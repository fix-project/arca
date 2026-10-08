extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::{
    ops::{Deref, DerefMut},
    slice::SliceIndex,
};
use serde::{Deserialize, Serialize, Serializer};

use crate::{BuddyAllocator, refcnt::RefCnt};

mod sealed {
    pub trait Bytes {}
    impl Bytes for [u8] {}
    impl<const N: usize> Bytes for [u8; N] {}
}

/// Buddy-backed storage or a borrow of it. Serialization sends only its offset and length.
/// Ownership stays with the sender, which must keep the buffer alive until completion.
pub struct Buffer<T> {
    storage: T,
}

impl<T> Buffer<T> {
    /// # Safety
    /// Every dereference of `storage` throughout its lifetime must refer to initialized
    /// Buddy-backed memory.
    pub unsafe fn from_buddy(storage: T) -> Self {
        Self { storage }
    }

    pub fn into_inner(self) -> T {
        self.storage
    }
}

impl<T: ?Sized + sealed::Bytes> From<Box<T, BuddyAllocator>> for Buffer<Box<T, BuddyAllocator>> {
    fn from(storage: Box<T, BuddyAllocator>) -> Self {
        Self { storage }
    }
}

impl<T: ?Sized + sealed::Bytes> From<RefCnt<T>> for Buffer<RefCnt<T>> {
    fn from(storage: RefCnt<T>) -> Self {
        Self { storage }
    }
}

impl From<Vec<u8, BuddyAllocator>> for Buffer<Vec<u8, BuddyAllocator>> {
    fn from(storage: Vec<u8, BuddyAllocator>) -> Self {
        Self { storage }
    }
}

impl<T: Deref> Deref for Buffer<T>
where
    T::Target: AsRef<[u8]>,
{
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.storage.deref().as_ref()
    }
}

impl<T: DerefMut> DerefMut for Buffer<T>
where
    T::Target: AsRef<[u8]> + AsMut<[u8]>,
{
    fn deref_mut(&mut self) -> &mut [u8] {
        self.storage.deref_mut().as_mut()
    }
}

impl<T> Buffer<T>
where
    Self: Deref<Target = [u8]>,
{
    pub fn slice<I: SliceIndex<[u8], Output = [u8]>>(&self, index: I) -> Buffer<&[u8]> {
        Buffer {
            storage: &self[index],
        }
    }
}

impl<T> Buffer<T>
where
    Self: DerefMut<Target = [u8]>,
{
    pub fn slice_mut<I: SliceIndex<[u8], Output = [u8]>>(&mut self, index: I) -> Buffer<&mut [u8]> {
        Buffer {
            storage: &mut self[index],
        }
    }
}

impl<T> Serialize for Buffer<T>
where
    Self: Deref<Target = [u8]>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Region {
            offset: if self.is_empty() {
                0
            } else {
                BuddyAllocator.to_offset(self.as_ptr())
            },
            len: self.len(),
        }
        .serialize(serializer)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Region {
    offset: usize,
    len: usize,
}

impl Region {
    fn is_valid(&self) -> bool {
        self.len <= isize::MAX as usize
            && self
                .offset
                .checked_add(self.len)
                .is_some_and(|end| end <= BuddyAllocator.len())
    }

    /// # Safety
    /// The sender must keep the region initialized and immutably borrowed until completion.
    pub unsafe fn as_slice(&self) -> &[u8] {
        assert!(self.is_valid(), "buffer is outside Buddy memory");
        unsafe { core::slice::from_raw_parts(BuddyAllocator.from_offset(self.offset), self.len) }
    }

    /// # Safety
    /// The sender must keep the region initialized and exclusively borrowed until completion.
    pub unsafe fn as_mut_slice(&mut self) -> &mut [u8] {
        assert!(self.is_valid(), "buffer is outside Buddy memory");
        unsafe {
            core::slice::from_raw_parts_mut(BuddyAllocator.from_offset(self.offset), self.len)
        }
    }
}
