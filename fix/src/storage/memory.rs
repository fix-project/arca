extern crate alloc;

use super::*;
use alloc::vec::Vec;
use alloc::boxed::Box;
use kernel::kthread::KMutex;
use bitint::U48;

/// An object store which stores its data in RAM.
#[derive(Debug, Default)]
pub struct MemoryStorage{
    blobs: KMutex<Vec<Box<[u8]>>>,
    trees: KMutex<Vec<Box<[Handle]>>>,
}

impl Storage for MemoryStorage {
    fn add_blob(&self, data: &[u8]) -> Blob {
        let mut blobs = self.blobs.lock();
        let i = blobs.len();
        let len = data.len();
        blobs.push(data.into());
        let mut name = [0; 24];
        name[0..8].copy_from_slice(&usize::to_le_bytes(i));
        unsafe {
            BlobName::new(RawName {
                name,
                size: U48::new(len as u64).unwrap(),
                meta: 0,
            }).into()
        }
    }

    fn add_tree(&self, data: &[Handle]) -> Tree {
        let mut trees = self.trees.lock();
        let i = trees.len();
        let len = data.len();
        trees.push(data.into());
        let mut name = [0; 24];
        name[0..8].copy_from_slice(&usize::to_le_bytes(i));
        unsafe {
            TreeName::new(RawName {
                name,
                size: U48::new(len as u64).unwrap(),
                meta: 0,
            }).into()
        }
    }

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>> {
        let blobs = self.blobs.lock();
        let mut i = [0; 8];
        i.copy_from_slice(&BlobName::from(name).name().name[0..8]);
        let i = usize::from_le_bytes(i);
        blobs.get(i).cloned()
    }

    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>> {
        let trees = self.trees.lock();
        let mut i = [0; 8];
        i.copy_from_slice(&TreeName::from(name).name().name[0..8]);
        let i = usize::from_le_bytes(i);
        trees.get(i).cloned()
    }
}
