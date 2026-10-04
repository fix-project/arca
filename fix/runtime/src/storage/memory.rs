extern crate alloc;

use super::*;
use alloc::boxed::Box;
use alloc::vec::Vec;
use bitint::U48;
use hashbrown::HashMap;
use kernel::kthread::KMutex;

/// An object store which stores its data in RAM.  Names are indices into the tables; the indices
/// are stored inverted for visual distinctiveness.
#[derive(Debug, Default)]
pub struct MemoryStorage {
    blobs: KMutex<Vec<Box<[u8]>>>,
    trees: KMutex<Vec<Box<[Handle]>>>,
    canonicals: KMutex<HashMap<CanonicalHandle, usize>>,
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
        unsafe { BlobName::new(PotentiallyCanonicalName::Local(raw)).into() }
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

    fn get_blob(&self, blob: Blob) -> Option<Box<[u8]>> {
        let name = match blob {
            Blob::Blob(name) => name,
            Blob::Literal(name) => return Some(name.bytes().into()),
        };

        let index = match name.name() {
            PotentiallyCanonicalName::Local(raw) => {
                let mut i = [0; 8];
                i.copy_from_slice(&raw.name[0..8]);
                !usize::from_le_bytes(i)
            },
            PotentiallyCanonicalName::Canonical(_) => {
                let canonical = CanonicalHandle::try_from(blob).expect("blob is canonical");
                let canonicals = self.canonicals.lock();
                canonicals.get(&canonical)?.clone()
            },
        };

        let blobs = self.blobs.lock();
        blobs.get(index).cloned()
    }

    fn get_tree(&self, name: Tree) -> Option<Box<[Handle]>> {
        let trees = self.trees.lock();
        let mut i = [0; 8];
        i.copy_from_slice(&TreeName::from(name).name().name[0..8]);
        let i = !usize::from_le_bytes(i);
        trees.get(i).cloned()
    }

    fn canonicalize_blob(&self, blob: Blob) -> Option<CanonicalHandle> {
        if blob.is_canonical() {
            return Some(unsafe { CanonicalHandle::new(blob.into()) })
        }

        // Everything at this point is a local, non-literal blob.
        // We need to get the memory index for the blob to insert into the canonicals map
        let Blob::Blob(local_name) = blob else { unreachable!() };
        let PotentiallyCanonicalName::Local(raw) = local_name.name() else { unreachable!() };
        let mut i = [0; 8];
        i.copy_from_slice(&raw.name[0..8]);
        let index = !usize::from_le_bytes(i);

        // Look up in self.blobs directly instead of self.get_blob to avoid recomputing index,
        // we already need the index later when we store it in the map anyway
        let blobs = self.blobs.lock();
        let bytes = blobs.get(index)?;
        
        // Take the first 24 bytes of blake3 content hash
        let hash = blake3::hash(bytes.as_ref());
        let len = bytes.len();

        // Finish borrowing `bytes` from blobs vec
        drop(blobs);

        let mut name = [0u8; 24];
        name.copy_from_slice(&hash.as_bytes()[..24]);
        let canonicalized = PotentiallyCanonicalName::Canonical(RawName {
            name,
            size: U48::new(len as u64).unwrap(),
            meta: 0,
        });

        let canonicalized_blob = Blob::Blob(unsafe { BlobName::new(canonicalized) });
        let canonical_handle = unsafe { CanonicalHandle::new(canonicalized_blob.into()) };
        self.canonicals.lock().entry(canonical_handle).or_insert(index);
        Some(canonical_handle)
    }

    fn canonicalize_tree(&self, _tree: Tree) -> Option<CanonicalHandle> {
        todo!();
    }
}
