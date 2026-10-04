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
    fn add_blob(&self, data: &[u8]) -> Blob {
        let mut blobs = self.blobs.lock();
        let i = blobs.len();
        let len = data.len();
        if len < 30 {
            return Blob::Literal(LiteralName::new(data));
        }
        blobs.push(data.into());
        let mut name = [0; 24];
        name[0..8].copy_from_slice(&usize::to_le_bytes(!i));
        let raw = RawName {
            name,
            size: U48::new(len as u64).unwrap(),
            meta: 0,
        };
        unsafe { BlobName::new(PotentiallyConincalName::Local(raw)).into() }
    }

    fn add_tree(&self, data: &[Handle]) -> Tree {
        let mut trees = self.trees.lock();
        let i = trees.len();
        let len = data.len();
        trees.push(data.into());
        let mut name = [0; 24];
        name[0..8].copy_from_slice(&usize::to_le_bytes(!i));
        unsafe {
            TreeName::new(RawName {
                name,
                size: U48::new(len as u64).unwrap(),
                meta: 0,
            })
            .into()
        }
    }

    fn get_blob(&self, name: Blob) -> Option<Box<[u8]>> {
        let blobs = self.blobs.lock();
        let mut i = [0; 8];
        let name = match name {
            Blob::Blob(name) => name,
            Blob::Literal(name) => return Some(name.bytes().into()),
        };

        let raw = match name.name() {
            PotentiallyConincalName::Local(raw) => raw,
            PotentiallyConincalName::Canonical(_) => todo!("get blob for canonical name"),
        };
        i.copy_from_slice(&raw.name[0..8]);
        let i = !usize::from_le_bytes(i);
        blobs.get(i).cloned()
    }

    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>> {
        let trees = self.trees.lock();
        let mut i = [0; 8];
        i.copy_from_slice(&TreeName::from(name).name().name[0..8]);
        let i = !usize::from_le_bytes(i);
        trees.get(i).cloned()
    }

    fn canonicalize_blob(&self, blob: Blob) -> Option<CanonicalHandle> {
        let bytes = self.get_blob(blob)?;
        let len = bytes.len();
        let is_literal = len < 30;
        let canonicalized_blob = if is_literal {
            blob
        } else {
            // Take the first 24 bytes of blake3 content hash
            let hash = blake3::hash(bytes.as_ref());
            let mut name = [0u8; 24];
            name.copy_from_slice(&hash.as_bytes()[..24]);

            let canonicalized = PotentiallyConincalName::Canonical(RawName {
                name,
                size: U48::new(len as u64).unwrap(),
                meta: 0,
            });
            Blob::Blob(unsafe { BlobName::new(canonicalized) })
        };

        Some(CanonicalHandle(Handle::Object(Object::Blob(
            canonicalized_blob,
        ))))
    }

    fn canonicalize_tree(&self, _tree: Tree) -> Option<CanonicalHandle> {
        todo!();
    }
}
