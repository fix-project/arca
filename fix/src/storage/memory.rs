extern crate alloc;

use super::*;
use alloc::boxed::Box;
use alloc::vec::Vec;
use bitint::U48;
use kernel::kthread::KMutex;

/// An object store which stores its data in RAM.  Names are indices into the tables; the indices
/// are stored inverted for visual distinctiveness.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    blobs: KMutex<Vec<Box<[u8]>>>,
    trees: KMutex<Vec<Box<[Handle]>>>,
}

impl Storage for MemoryStorage {
    fn add_blob(&self, data: &[u8]) -> Result<Blob, StorageError> {
        let mut blobs = self.blobs.lock();
        let i = blobs.len();
        let len = data.len();
        if len < 30 {
            return Ok(Blob::Literal(LiteralName::new(data)));
        }
        blobs.push(data.into());
        let mut name = [0; 24];
        name[0..8].copy_from_slice(&usize::to_le_bytes(!i));
        Ok(unsafe {
            BlobName::new(RawName {
                name,
                size: U48::new(len as u64).unwrap(),
                meta: RawName::MACHINE_NAME,
            })
            .into()
        })
    }

    fn add_tree(&self, data: &[Handle]) -> Result<Tree, StorageError> {
        let mut trees = self.trees.lock();
        let i = trees.len();
        let len = data.len();
        trees.push(data.into());
        let mut name = [0; 24];
        name[0..8].copy_from_slice(&usize::to_le_bytes(!i));
        Ok(unsafe {
            TreeName::new(RawName {
                name,
                size: U48::new(len as u64).unwrap(),
                meta: RawName::MACHINE_NAME,
            })
            .into()
        })
    }

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>> {
        let blobs = self.blobs.lock();
        let mut i = [0; 8];
        let name = match name {
            Blob::Blob(name) => name,
            Blob::Literal(name) => return Some(name.bytes().into()),
        };
        if !name.is_machine() {
            return None;
        }
        i.copy_from_slice(&name.name().name[0..8]);
        let i = !usize::from_le_bytes(i);
        blobs.get(i).cloned()
    }

    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>> {
        let trees = self.trees.lock();
        let mut i = [0; 8];
        let name = TreeName::from(name);
        if !name.is_machine() {
            return None;
        }
        i.copy_from_slice(&name.name().name[0..8]);
        let i = !usize::from_le_bytes(i);
        trees.get(i).cloned()
    }

    fn import(&self, _from: &dyn Storage, _handle: Handle) -> Result<Handle, ImportError> {
        panic!("MemoryStorage::import is not implemented")
    }
}
