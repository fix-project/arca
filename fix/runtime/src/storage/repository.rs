use super::*;
use crate::repository::{Repository, RepositoryError};
use alloc::{sync::Arc, vec::Vec};
use hashbrown::HashMap;
use kernel::kthread::KMutex;

type InputCache = HashMap<[u8; 32], Arc<KMutex<Option<ImmutableBytes>>>>;

#[derive(Debug)]
pub struct RepositoryStorage {
    repository: Repository,
    memory: memory::MemoryStorage,
    inputs: KMutex<InputCache>,
}

impl RepositoryStorage {
    pub fn new(repository: Repository) -> Self {
        Self {
            repository,
            memory: memory::MemoryStorage::default(),
            inputs: KMutex::new(HashMap::new()),
        }
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
    }

    fn backing(&self, object: Object) -> Result<ImmutableBytes, StorageError> {
        let key = *object.as_handle().as_bytes();
        let cell = self.inputs.lock().entry(key).or_default().clone();
        let mut cached = cell.lock();
        if let Some(bytes) = cached.as_ref() {
            return Ok(bytes.clone());
        }
        let bytes = match object.view() {
            ObjectView::Blob(blob) => self.repository.read_blob(*blob),
            ObjectView::Tree(tree) => self.repository.read_tree(*tree).map(|children| {
                children
                    .iter()
                    .flat_map(|child| *child.as_bytes())
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }),
        }
        .map_err(|_| StorageError::NotFound)?;
        let bytes = ImmutableBytes::new(&bytes);
        *cached = Some(bytes.clone());
        Ok(bytes)
    }

    pub fn save(&self, handle: Handle) -> Result<Handle, RepositoryError> {
        let handle: Handle = self
            .memory
            .canonicalize(handle)
            .map_err(RepositoryError::Storage)?
            .into();
        self.publish(handle)?;
        Ok(handle)
    }

    fn publish(&self, handle: Handle) -> Result<(), RepositoryError> {
        if self.repository.lookup(handle.as_bytes()).is_ok() {
            if let Ok(tree) = Tree::try_from(handle)
                && tree.is_tag()
            {
                self.repository.index_tag(tree)?;
            }
            return Ok(());
        }
        let object = match handle.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            HandleView::Thunk(thunk) => return self.publish_thunk(*thunk),
            HandleView::Encode(encode) => {
                return match encode.view() {
                    EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => {
                        self.publish_thunk(thunk)
                    }
                };
            }
        };
        let bytes = match object.view() {
            ObjectView::Blob(blob) => self.get_blob(*blob).map_err(RepositoryError::Storage)?,
            ObjectView::Tree(tree) => {
                let children = self.get_tree(*tree).map_err(RepositoryError::Storage)?;
                for child in &children {
                    self.publish(*child)?;
                }
                children
                    .iter()
                    .flat_map(|child| *child.as_bytes())
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            }
        };
        self.repository.publish(object.into(), &bytes)
    }

    fn publish_thunk(&self, thunk: Thunk) -> Result<(), RepositoryError> {
        self.publish(match thunk.view() {
            ThunkView::Identification(reference) => reference.into(),
            ThunkView::Application(reference) | ThunkView::Selection(reference) => reference.into(),
        })
    }
}

impl Storage for RepositoryStorage {
    fn canonicalize(&self, handle: Handle) -> Result<CanonicalHandle, StorageError> {
        self.memory.canonicalize(handle)
    }

    fn add_blob(&self, bytes: &[u8]) -> Blob {
        self.memory.add_blob(bytes)
    }
    fn add_blob_backing(&self, data: ImmutableBytes) -> Blob {
        self.memory.add_blob_backing(data)
    }
    fn add_tree(&self, children: &[Handle]) -> Tree {
        self.memory.add_tree(children)
    }
    unsafe fn add_tree_backing(&self, data: ImmutableBytes) -> Tree {
        unsafe { self.memory.add_tree_backing(data) }
    }
    fn get_blob(&self, blob: Blob) -> Result<Box<[u8]>, StorageError> {
        Ok(self.get_blob_backing(blob)?.to_bytes())
    }
    fn get_tree(&self, tree: Tree) -> Result<Box<[Handle]>, StorageError> {
        let bytes = self.get_tree_backing(tree)?.to_bytes();
        bytes
            .as_chunks::<32>()
            .0
            .iter()
            .map(|bytes| unsafe { Handle::parse(*bytes) }.map_err(|_| StorageError::NotFound))
            .collect()
    }
    fn get_blob_backing(&self, blob: Blob) -> Result<ImmutableBytes, StorageError> {
        self.memory.get_blob_backing(blob).or_else(|error| {
            if blob.as_handle().is_canonical() {
                self.backing(blob.object())
            } else {
                Err(error)
            }
        })
    }
    fn get_tree_backing(&self, tree: Tree) -> Result<ImmutableBytes, StorageError> {
        self.memory.get_tree_backing(tree).or_else(|error| {
            if tree.as_handle().is_canonical() {
                self.backing(tree.object())
            } else {
                Err(error)
            }
        })
    }
    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError> {
        self.memory.canonicalize_blob(blob)
    }
    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError> {
        self.memory.canonicalize_tree(tree)
    }
}
