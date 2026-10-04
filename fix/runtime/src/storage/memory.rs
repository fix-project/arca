extern crate alloc;

use super::*;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};
use hashbrown::HashMap;
use kernel::kthread::KMutex;

static NEXT_STORAGE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct MemoryStorage {
    id: u64,
    blobs: KMutex<Vec<ImmutableBytes>>,
    trees: KMutex<Vec<(ImmutableBytes, bool)>>,
    canonicals: KMutex<HashMap<CanonicalHandle, usize>>,
    locals: KMutex<HashMap<[u8; HANDLE_SIZE], CanonicalHandle>>,
}

impl Default for MemoryStorage {
    fn default() -> Self {
        let id = NEXT_STORAGE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("storage identifiers exhausted");
        Self {
            id,
            blobs: KMutex::new(Vec::new()),
            trees: KMutex::new(Vec::new()),
            canonicals: KMutex::new(HashMap::new()),
            locals: KMutex::new(HashMap::new()),
        }
    }
}

impl MemoryStorage {
    fn index(&self, handle: &Handle) -> Result<usize, StorageError> {
        match handle.name() {
            Name::Canonical(_) => self
                .canonicals
                .lock()
                .get(&unsafe { CanonicalHandle::new(*handle) })
                .copied()
                .ok_or(StorageError::NotFound),
            Name::Machine { index, storage_id } if storage_id == self.id => {
                usize::try_from(index).map_err(|_| StorageError::NotFound)
            }
            Name::Machine { .. } => Err(StorageError::WrongStorage),
            _ => Err(StorageError::UnsupportedName),
        }
    }

    fn canonicalize_object(
        &self,
        object: Object,
        visiting: &mut Vec<Handle>,
    ) -> Result<CanonicalHandle, StorageError> {
        match object.view() {
            ObjectView::Blob(blob) => self.canonicalize_blob(*blob),
            ObjectView::Tree(tree) => self.canonicalize_tree_inner(*tree, visiting),
        }
    }

    fn canonicalize_tree_inner(
        &self,
        tree: Tree,
        visiting: &mut Vec<Handle>,
    ) -> Result<CanonicalHandle, StorageError> {
        if let Ok(canonical) = CanonicalHandle::try_from(tree) {
            return Ok(canonical);
        }
        let handle = tree.handle();
        if let Some(canonical) = self.locals.lock().get(handle.as_bytes()).copied() {
            return Ok(canonical);
        }
        if visiting
            .iter()
            .any(|ancestor| ancestor.as_bytes() == handle.as_bytes())
        {
            return Err(StorageError::Cycle);
        }
        visiting.push(handle);
        let mut children = self.get_tree(tree)?;
        for child in &mut children {
            *child = self.canonicalize_handle(*child, visiting)?;
        }
        let canonical = Tree::canonical(&children).expect("canonical children");
        let canonical_handle = CanonicalHandle::try_from(canonical).unwrap();
        let mut canonicals = self.canonicals.lock();
        let index = *canonicals.entry(canonical_handle).or_insert_with(|| {
            let stored = self.add_tree(&children);
            self.index(stored.as_handle()).expect("new tree index")
        });
        let tagged = CanonicalHandle::try_from(canonical.tag_descriptor()).unwrap();
        canonicals.entry(tagged).or_insert(index);
        drop(canonicals);
        visiting.pop();
        let canonical = if tree.is_tag() {
            tagged
        } else {
            canonical_handle
        };
        self.locals.lock().insert(*handle.as_bytes(), canonical);
        Ok(canonical)
    }

    fn canonicalize_handle(
        &self,
        handle: Handle,
        visiting: &mut Vec<Handle>,
    ) -> Result<Handle, StorageError> {
        match handle.view() {
            HandleView::Object(object) => Ok(self.canonicalize_object(*object, visiting)?.into()),
            HandleView::Ref(reference) => {
                let canonical: Handle = self
                    .canonicalize_object(reference.object_descriptor(), visiting)?
                    .into();
                Ok(Object::try_from(canonical).unwrap().into_ref().into())
            }
            HandleView::Thunk(thunk) => Ok(self.canonicalize_thunk(*thunk, visiting)?.into()),
            HandleView::Encode(encode) => match encode.view() {
                EncodeView::Strict(thunk) => {
                    Ok(self.canonicalize_thunk(thunk, visiting)?.strict().into())
                }
                EncodeView::Shallow(thunk) => {
                    Ok(self.canonicalize_thunk(thunk, visiting)?.shallow().into())
                }
            },
        }
    }

    fn canonicalize_thunk(
        &self,
        thunk: Thunk,
        visiting: &mut Vec<Handle>,
    ) -> Result<Thunk, StorageError> {
        match thunk.view() {
            ThunkView::Identification(reference) => {
                let canonical = self.canonicalize_handle(reference.into(), visiting)?;
                Ok(Ref::try_from(canonical).unwrap().identification())
            }
            ThunkView::Application(reference) => {
                let canonical = self.canonicalize_handle(reference.into(), visiting)?;
                Ok(TreeRef::try_from(canonical).unwrap().application())
            }
            ThunkView::Selection(reference) => {
                let canonical = self.canonicalize_handle(reference.into(), visiting)?;
                Ok(TreeRef::try_from(canonical).unwrap().selection())
            }
        }
    }
}

impl Storage for MemoryStorage {
    fn add_blob(&self, data: &[u8]) -> Blob {
        if data.len() <= 30 {
            return Blob::literal(data).expect("literal size checked");
        }
        let mut blobs = self.blobs.lock();
        let handle = unsafe {
            Blob::named(
                Name::Machine {
                    index: blobs.len() as u64,
                    storage_id: self.id,
                },
                data.len() as u64,
                true,
            )
        }
        .expect("blob exceeds handle length");
        blobs.push(ImmutableBytes::new(data));
        handle
    }

    fn add_tree(&self, data: &[Handle]) -> Tree {
        let eq = data.iter().all(Handle::is_eq);
        let mut trees = self.trees.lock();
        let bytes = (data.len() as u64)
            .checked_mul(HANDLE_SIZE as u64)
            .expect("tree length overflow");
        let handle = unsafe {
            Tree::named(
                Name::Machine {
                    index: trees.len() as u64,
                    storage_id: self.id,
                },
                bytes,
                eq,
            )
        }
        .expect("tree exceeds handle length");
        let bytes: Vec<_> = data
            .iter()
            .flat_map(|handle| handle.as_bytes().iter().copied())
            .collect();
        trees.push((ImmutableBytes::new(&bytes), eq));
        handle
    }

    fn get_blob(&self, name: Blob) -> Result<Box<[u8]>, StorageError> {
        if let Some(bytes) = name.literal_bytes() {
            return Ok(bytes.into());
        }
        Ok(self.get_blob_backing(name)?.to_bytes())
    }

    fn get_tree(&self, name: Tree) -> Result<Box<[Handle]>, StorageError> {
        let bytes = self.get_tree_backing(name)?.to_bytes();
        Ok(bytes
            .as_chunks::<HANDLE_SIZE>()
            .0
            .iter()
            .map(|bytes| unsafe { Handle::parse(*bytes) }.expect("stored tree entry"))
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }

    fn get_blob_backing(&self, name: Blob) -> Result<ImmutableBytes, StorageError> {
        if let Some(bytes) = name.literal_bytes() {
            return Ok(ImmutableBytes::new(bytes));
        }
        let index = self.index(name.as_handle())?;
        let blobs = self.blobs.lock();
        let data = blobs.get(index).ok_or(StorageError::NotFound)?;
        if data.len() as u64 != name.byte_len() {
            return Err(StorageError::LengthMismatch);
        }
        if name.as_handle().eq_bit() != Some(true) {
            return Err(StorageError::InvalidEq);
        }
        Ok(data.clone())
    }

    fn get_tree_backing(&self, name: Tree) -> Result<ImmutableBytes, StorageError> {
        let index = self.index(name.as_handle())?;
        let trees = self.trees.lock();
        let (data, eq) = trees.get(index).ok_or(StorageError::NotFound)?;
        if data.len() as u64 != name.byte_len() {
            return Err(StorageError::LengthMismatch);
        }
        if name.as_handle().eq_bit() != Some(*eq) {
            return Err(StorageError::InvalidEq);
        }
        Ok(data.clone())
    }

    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError> {
        if blob.as_handle().eq_bit() == Some(false) {
            return Err(StorageError::InvalidEq);
        }
        if let Ok(canonical) = CanonicalHandle::try_from(blob) {
            return Ok(canonical);
        }
        if let Some(canonical) = self.locals.lock().get(blob.as_handle().as_bytes()).copied() {
            return Ok(canonical);
        }
        let index = self.index(blob.as_handle())?;
        let blobs = self.blobs.lock();
        let bytes = blobs.get(index).ok_or(StorageError::NotFound)?;
        if blob.byte_len() != bytes.len() as u64 {
            return Err(StorageError::LengthMismatch);
        }
        let bytes = bytes.to_bytes();
        drop(blobs);

        let canonicalized_blob = Blob::canonical(&bytes).expect("stored blob length");
        let canonical_handle =
            CanonicalHandle::try_from(canonicalized_blob).expect("canonical blob");
        self.canonicals
            .lock()
            .entry(canonical_handle)
            .or_insert(index);
        self.locals
            .lock()
            .insert(*blob.as_handle().as_bytes(), canonical_handle);
        Ok(canonical_handle)
    }

    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError> {
        self.canonicalize_object(tree.object(), &mut Vec::new())
    }
}
