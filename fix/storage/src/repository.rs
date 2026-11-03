use super::*;
use crate::platform::Mutex;
use alloc::{sync::Arc, vec, vec::Vec};
use fixrepository::{Repository, RepositoryError};
use memory::ShardedMap;

#[derive(Debug)]
pub struct RepositoryStorage {
    repository: Repository,
    memory: memory::MemoryStorage,
    /// One gate per Object being read, so concurrent loads read it once.
    loading: ShardedMap<[u8; HANDLE_SIZE], Arc<Mutex<()>>>,
    resident: ShardedMap<[u8; HANDLE_SIZE], ()>,
}

impl AsRef<memory::MemoryStorage> for RepositoryStorage {
    fn as_ref(&self) -> &memory::MemoryStorage {
        &self.memory
    }
}

impl AsRef<memory::MemoryStorage> for Arc<RepositoryStorage> {
    fn as_ref(&self) -> &memory::MemoryStorage {
        &self.memory
    }
}

impl RepositoryStorage {
    pub fn new(repository: Repository) -> Self {
        Self {
            repository,
            memory: memory::MemoryStorage::default(),
            loading: ShardedMap::default(),
            resident: ShardedMap::default(),
        }
    }

    pub fn repository(&self) -> &Repository {
        &self.repository
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
        if self.repository.has_backing(object).unwrap_or(false) {
            if let Ok(tree) = Tree::try_from(handle)
                && tree.is_tag()
            {
                self.repository.index_tag(tree)?;
            }
            return Ok(());
        }
        if matches!(handle.view(), HandleView::Ref(_)) {
            let resident = match object.view() {
                ObjectView::Blob(blob) => self.memory.has_blob(*blob),
                ObjectView::Tree(tree) => self.memory.has_tree(*tree),
            };
            if !resident && self.repository.lookup(handle.as_bytes()).is_ok() {
                return Ok(());
            }
        }
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

    fn load_object(&self, object: Object) -> bool {
        let handle: Handle = object.into();
        if matches!(handle.name(), Name::Literal(_)) || self.memory.contains(handle) {
            return true;
        }
        if !matches!(handle.name(), Name::Canonical(_)) {
            return false;
        }
        let key = *handle.as_bytes();
        let gate = self
            .loading
            .with(&key, |loading| loading.entry(key).or_default().clone());
        let _loading = gate.lock();
        if self.memory.contains(handle) {
            return true;
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
        };
        if let Ok(bytes) = &bytes {
            unsafe {
                self.memory
                    .insert_canonical(object, ImmutableBytes::new(bytes));
            }
        }
        self.loading.with(&key, |loading| loading.remove(&key));
        bytes.is_ok()
    }

    fn publish_thunk(&self, thunk: Thunk) -> Result<(), RepositoryError> {
        self.publish(match thunk.view() {
            ThunkView::Identification(reference) => reference.into(),
            ThunkView::Application(reference) | ThunkView::Selection(reference) => reference.into(),
        })
    }
}

impl Storage for RepositoryStorage {
    fn identify(&self, bytes: &[u8; HANDLE_SIZE]) -> Result<Handle, StorageError> {
        self.memory.identify(bytes).or_else(|_| {
            self.repository
                .lookup(bytes)
                .map_err(|_| StorageError::NotFound)
        })
    }

    fn load(&self, handle: Handle) {
        if !matches!(handle.view(), HandleView::Object(_))
            || self.resident.with(handle.as_bytes(), |resident| {
                resident.contains_key(handle.as_bytes())
            })
        {
            return;
        }
        let mut pending = vec![(handle, None)];
        let mut visited = hashbrown::HashSet::new();
        let mut failures = 0usize;
        while let Some((handle, completed)) = pending.pop() {
            let key = *handle.as_bytes();
            if let Some(previous) = completed {
                if previous == failures {
                    self.resident
                        .with(&key, |resident| resident.insert(key, ()));
                }
                continue;
            }
            let HandleView::Object(object) = handle.view() else {
                continue;
            };
            if self
                .resident
                .with(&key, |resident| resident.contains_key(&key))
            {
                continue;
            }
            if !visited.insert(key) || !self.load_object(*object) {
                failures += 1;
                continue;
            }
            match object.view() {
                ObjectView::Blob(_) => {
                    self.resident
                        .with(&key, |resident| resident.insert(key, ()));
                }
                ObjectView::Tree(tree) => {
                    if let Some(children) = self.memory.present_children(*tree) {
                        pending.push((handle, Some(failures)));
                        pending.extend(children.into_vec().into_iter().map(|child| (child, None)));
                    } else {
                        failures += 1;
                    }
                }
            }
        }
    }

    fn has_blob(&self, name: Blob) -> bool {
        self.memory.has_blob(name) || self.repository.has_backing(name.object()).unwrap_or(false)
    }
    fn has_tree(&self, name: Tree) -> bool {
        self.memory.has_tree(name) || self.repository.has_backing(name.into()).unwrap_or(false)
    }

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
        self.get_tree_backing(tree)?.read_handles()
    }
    fn get_blob_backing(&self, blob: Blob) -> Result<ImmutableBytes, StorageError> {
        self.memory.get_blob_backing(blob).or_else(|_| {
            self.load(blob.into());
            self.memory.get_blob_backing(blob)
        })
    }
    fn get_tree_backing(&self, tree: Tree) -> Result<ImmutableBytes, StorageError> {
        self.memory.get_tree_backing(tree).or_else(|_| {
            self.load_object(tree.into());
            self.memory.get_tree_backing(tree)
        })
    }
    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError> {
        self.memory.canonicalize_blob(blob)
    }
    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError> {
        self.memory.canonicalize_tree(tree)
    }
}
