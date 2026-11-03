extern crate alloc;

use super::*;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bytes::Buf;
use core::hash::{BuildHasher, Hasher};
use core::sync::atomic::{AtomicU64, Ordering};
use hashbrown::HashSet;

mod table;
use crate::platform::Mutex as KMutex;
pub use table::ShardedMap;
use table::Table;

static NEXT_STORAGE_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub struct MemoryStorage {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    id: u64,
    next_blob: AtomicU64,
    next_tree: AtomicU64,
    blobs: Table<ImmutableBytes>,
    trees: Table<(TreeBacking, bool, bool)>,
    canonicals: ShardedMap<CanonicalHandle, usize>,
    locals: ShardedMap<[u8; HANDLE_SIZE], CanonicalHandle>,
    claims: ShardedMap<[[u8; HANDLE_SIZE]; 2], Vec<IndexedClaim>>,
    tree_hasher: hashbrown::DefaultHashBuilder,
    tree_index: ShardedMap<u64, Vec<Tree>>,
}

#[derive(Clone, Debug)]
struct IndexedClaim {
    coupon: Tree,
    author: [u8; HANDLE_SIZE],
    result: [u8; HANDLE_SIZE],
}

type TreeProducer = Box<dyn FnOnce(&MemoryStorage) -> ImmutableBytes + Send>;

#[derive(Clone, Debug)]
enum TreeBacking {
    Ready(ImmutableBytes),
    Deferred {
        len: usize,
        state: Arc<KMutex<DeferredTree>>,
    },
}

enum DeferredTree {
    Pending(Option<TreeProducer>),
    Ready(ImmutableBytes),
    Failed(StorageError),
}

impl core::fmt::Debug for DeferredTree {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Pending(_) => f.write_str("Pending"),
            Self::Ready(data) => f.debug_tuple("Ready").field(data).finish(),
            Self::Failed(error) => f.debug_tuple("Failed").field(error).finish(),
        }
    }
}

impl TreeBacking {
    fn len(&self) -> usize {
        match self {
            Self::Ready(bytes) => bytes.len(),
            Self::Deferred { len, .. } => *len,
        }
    }

    fn get(
        &self,
        storage: &MemoryStorage,
        comparable: bool,
        value: bool,
    ) -> Result<ImmutableBytes, StorageError> {
        let (len, state) = match self {
            Self::Ready(bytes) => return Ok(bytes.clone()),
            Self::Deferred { len, state } => (*len, state),
        };
        let mut state = state.lock();
        if let DeferredTree::Pending(produce) = &mut *state {
            let bytes = produce.take().expect("recursive tree materialization")(storage);
            let result = if bytes.len() != len {
                Err(StorageError::LengthMismatch)
            } else {
                let (actual_comparable, actual_value) = tree_properties(&bytes);
                if actual_comparable != comparable {
                    Err(StorageError::InvalidComparisonBit)
                } else if actual_value != value {
                    Err(StorageError::InvalidValueBit)
                } else {
                    Ok(bytes)
                }
            };
            *state = match result {
                Ok(bytes) => DeferredTree::Ready(bytes),
                Err(error) => DeferredTree::Failed(error),
            };
        }
        match &*state {
            DeferredTree::Ready(bytes) => Ok(bytes.clone()),
            DeferredTree::Failed(error) => Err(*error),
            DeferredTree::Pending(_) => unreachable!(),
        }
    }
}

fn tree_properties(data: &ImmutableBytes) -> (bool, bool) {
    assert!(data.len().is_multiple_of(HANDLE_SIZE));
    let mut comparable = true;
    let mut value = true;
    data.for_each_chunk(|bytes| {
        for bytes in bytes.as_chunks::<HANDLE_SIZE>().0 {
            let handle = unsafe { Handle::parse(*bytes) }.expect("invalid tree entry");
            comparable &= handle.is_comparable();
            value &= handle.is_value();
        }
    });
    (comparable, value)
}

impl AsRef<MemoryStorage> for MemoryStorage {
    fn as_ref(&self) -> &MemoryStorage {
        self
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        let id = NEXT_STORAGE_ID
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("storage identifiers exhausted");
        Self {
            inner: Arc::new(Inner {
                id,
                next_blob: AtomicU64::new(0),
                next_tree: AtomicU64::new(0),
                blobs: Table::default(),
                trees: Table::default(),
                canonicals: ShardedMap::default(),
                locals: ShardedMap::default(),
                claims: ShardedMap::default(),
                tree_hasher: hashbrown::DefaultHashBuilder::default(),
                tree_index: ShardedMap::default(),
            }),
        }
    }
}

impl MemoryStorage {
    fn cache_alias(&self, handle: Handle, canonical: CanonicalHandle) {
        let key = *handle.as_bytes();
        self.inner
            .locals
            .with(&key, |locals| locals.insert(key, canonical));
    }

    fn alias(&self, handle: &Handle) -> Option<CanonicalHandle> {
        self.inner.locals.with(handle.as_bytes(), |locals| {
            locals.get(handle.as_bytes()).copied()
        })
    }

    pub fn contains(&self, handle: Handle) -> bool {
        if handle.is_empty() && handle.same_encoding(&Tree::empty().into()) {
            return true;
        }
        let Ok(index) = self.index(&handle) else {
            return false;
        };
        if handle.is_tree_data() {
            self.inner.trees.get(index, |_| ()).is_some()
        } else {
            self.inner.blobs.get(index, |_| ()).is_some()
        }
    }

    pub fn index_claim(&self, source: &dyn Storage, value: Handle) -> Result<(), StorageError> {
        let Ok(coupon) = Tree::try_from(value) else {
            return Ok(());
        };
        if !coupon.is_tag() {
            return Ok(());
        }
        let fields = source.get_tree(coupon)?;
        let [author, reference] = fields.as_ref() else {
            return Ok(());
        };
        let Ok(reference) = TreeRef::try_from(*reference) else {
            return Ok(());
        };
        let claim = source.get_tree(reference.object_descriptor())?;
        let [statement, input, result] = claim.as_ref() else {
            return Ok(());
        };
        let key = [*statement.as_bytes(), *input.as_bytes()];
        let author = *author.as_bytes();
        let result = *result.as_bytes();
        self.inner.claims.with(&key, |claims| {
            let coupons = claims.entry(key).or_default();
            if !coupons
                .iter()
                .any(|saved| saved.author == author && saved.result == result)
            {
                coupons.push(IndexedClaim {
                    coupon,
                    author,
                    result,
                });
            }
        });
        Ok(())
    }

    /// Stores an Object read from elsewhere under its canonical name.
    ///
    /// # Safety
    /// `bytes` must be the contents that `object`'s canonical name denotes.
    pub unsafe fn insert_canonical(&self, object: Object, bytes: ImmutableBytes) {
        if object.is_empty() && object.as_handle().same_encoding(&Tree::empty().into()) {
            return;
        }
        let index = match object.view() {
            ObjectView::Blob(_) => {
                let index = self.next(&self.inner.next_blob);
                self.inner.blobs.insert(index, bytes);
                index
            }
            ObjectView::Tree(_) => {
                let index = self.next(&self.inner.next_tree);
                let (comparable, value) = tree_properties(&bytes);
                self.inner
                    .trees
                    .insert(index, (TreeBacking::Ready(bytes), comparable, value));
                index
            }
        };
        let canonical = unsafe { CanonicalHandle::new(object.into()) };
        self.inner.canonicals.with(&canonical, |canonicals| {
            canonicals.entry(canonical).or_insert(index);
        });
    }

    fn next(&self, counter: &AtomicU64) -> usize {
        counter
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("storage identifiers exhausted") as usize
    }

    /// The children of a tree whose contents are present, without materializing a
    /// deferred tree.
    pub fn present_children(&self, tree: Tree) -> Option<Box<[Handle]>> {
        if tree.is_empty() && tree.as_handle().same_encoding(&Tree::empty().into()) {
            return Some(Box::new([]));
        }
        let index = self.index(tree.as_handle()).ok()?;
        let bytes = self
            .inner
            .trees
            .get(index, |(backing, _, _)| match backing {
                TreeBacking::Ready(bytes) => Some(bytes.clone()),
                TreeBacking::Deferred { state, .. } => match &*state.lock() {
                    DeferredTree::Ready(bytes) => Some(bytes.clone()),
                    _ => None,
                },
            })??;
        Some(bytes.read_handles().expect("stored tree entries"))
    }

    pub fn claims(&self, key: [Handle; 2]) -> Vec<Tree> {
        let key = key.map(|handle| *handle.as_bytes());
        self.inner.claims.with(&key, |claims| {
            claims
                .get(&key)
                .map(|coupons| coupons.iter().map(|claim| claim.coupon).collect())
                .unwrap_or_default()
        })
    }

    pub fn blob_bytes(&self) -> usize {
        let mut total = 0;
        let end = self.inner.next_blob.load(Ordering::Acquire) as usize;
        self.inner.blobs.for_each(end, |bytes| total += bytes.len());
        total
    }

    pub fn tree_bytes(&self) -> usize {
        let mut total = 0;
        let end = self.inner.next_tree.load(Ordering::Acquire) as usize;
        self.inner
            .trees
            .for_each(end, |(backing, _, _)| total += backing.len());
        total
    }

    /// # Safety
    /// The producer must return valid handles with the declared length and flags.
    /// Its storage dependencies must be acyclic.
    pub unsafe fn defer_tree(
        &self,
        elements: usize,
        comparable: bool,
        value: bool,
        produce: impl FnOnce(&MemoryStorage) -> ImmutableBytes + Send + 'static,
    ) -> Tree {
        if elements == 0 {
            return Tree::empty();
        }
        self.tree(
            TreeBacking::Deferred {
                len: elements
                    .checked_mul(HANDLE_SIZE)
                    .expect("tree length overflow"),
                state: Arc::new(KMutex::new(DeferredTree::Pending(Some(Box::new(produce))))),
            },
            comparable,
            value,
        )
    }

    fn tree(&self, data: TreeBacking, comparable: bool, value: bool) -> Tree {
        if data.len() == 0 {
            return Tree::empty();
        }
        let index = self.next(&self.inner.next_tree);
        let handle = unsafe {
            Tree::named(
                Name::Machine {
                    index: index as u64,
                    storage_id: self.inner.id,
                },
                data.len() as u64,
                comparable,
                value,
            )
        }
        .expect("tree exceeds handle length");
        self.inner.trees.insert(index, (data, comparable, value));
        handle
    }

    fn index(&self, handle: &Handle) -> Result<usize, StorageError> {
        match handle.name() {
            Name::Canonical(_) => self
                .inner
                .canonicals
                .with(handle.as_bytes(), |canonicals| {
                    canonicals.get(handle.as_bytes()).copied()
                })
                .ok_or(StorageError::NotFound),
            Name::Machine { index, storage_id } if storage_id == self.inner.id => {
                usize::try_from(index).map_err(|_| StorageError::NotFound)
            }
            Name::Machine { .. } => Err(StorageError::WrongStorage),
            _ => Err(StorageError::UnsupportedName),
        }
    }

    fn canonicalize_object(&self, object: Object) -> Result<CanonicalHandle, StorageError> {
        match object.view() {
            ObjectView::Blob(blob) => self.canonicalize_blob(*blob),
            ObjectView::Tree(tree) => self.canonicalize_tree_inner(*tree),
        }
    }

    fn referenced_tree(handle: Handle) -> Option<Tree> {
        let object = match handle.view() {
            HandleView::Object(object) => *object,
            HandleView::Ref(reference) => reference.object_descriptor(),
            HandleView::Thunk(thunk) => match thunk.view() {
                ThunkView::Identification(reference) => reference.object_descriptor(),
                ThunkView::Application(reference) | ThunkView::Selection(reference) => {
                    reference.object_descriptor().object()
                }
            },
            HandleView::Encode(encode) => {
                let thunk = match encode.view() {
                    EncodeView::Strict(thunk) | EncodeView::Shallow(thunk) => thunk,
                };
                return Self::referenced_tree(thunk.into());
            }
        };
        match object.view() {
            ObjectView::Tree(tree) => Some(*tree),
            ObjectView::Blob(_) => None,
        }
    }

    fn canonicalize_tree_inner(&self, tree: Tree) -> Result<CanonicalHandle, StorageError> {
        if let Ok(canonical) = CanonicalHandle::try_from(tree) {
            return Ok(canonical);
        }
        let handle = tree.handle();
        if let Some(canonical) = self.alias(&handle) {
            return Ok(canonical);
        }
        struct Frame {
            tree: Tree,
            children: Box<[Handle]>,
            next: usize,
        }
        let mut active = HashSet::new();
        active.insert(*handle.as_bytes());
        let mut stack = alloc::vec![Frame {
            tree,
            children: self.get_tree(tree)?,
            next: 0,
        }];
        while !stack.is_empty() {
            let next = {
                let frame = stack.last_mut().unwrap();
                if frame.next < frame.children.len() {
                    let child = frame.children[frame.next];
                    frame.next += 1;
                    Some(child)
                } else {
                    None
                }
            };
            if let Some(child) = next {
                if let Some(dependency) = Self::referenced_tree(child) {
                    let key = *dependency.as_handle().as_bytes();
                    if !dependency.as_handle().is_canonical()
                        && self.alias(dependency.as_handle()).is_none()
                    {
                        if !active.insert(key) {
                            return Err(StorageError::Cycle);
                        }
                        stack.push(Frame {
                            tree: dependency,
                            children: self.get_tree(dependency)?,
                            next: 0,
                        });
                    }
                }
                continue;
            }
            let mut frame = stack.pop().unwrap();
            for child in &mut frame.children {
                *child = self.canonicalize_handle(*child)?;
            }
            let canonical = Tree::canonical(&frame.children).expect("canonical children");
            let canonical_handle = CanonicalHandle::try_from(canonical).unwrap();
            let index = self.inner.canonicals.with(&canonical_handle, |canonicals| {
                *canonicals.entry(canonical_handle).or_insert_with(|| {
                    let stored = self.add_tree(&frame.children);
                    self.index(stored.as_handle()).expect("new tree index")
                })
            });
            let tagged = CanonicalHandle::try_from(canonical.tag_descriptor()).unwrap();
            if frame.tree.is_tag() {
                self.inner.canonicals.with(&tagged, |canonicals| {
                    canonicals.entry(tagged).or_insert(index);
                });
            }
            let canonical = if frame.tree.is_tag() {
                tagged
            } else {
                canonical_handle
            };
            self.cache_alias(frame.tree.handle(), canonical);
            active.remove(frame.tree.as_handle().as_bytes());
        }
        Ok(self.alias(&handle).unwrap())
    }

    fn canonicalize_handle(&self, handle: Handle) -> Result<Handle, StorageError> {
        match handle.view() {
            HandleView::Object(object) => Ok(self.canonicalize_object(*object)?.into()),
            HandleView::Ref(reference) => {
                let canonical: Handle = self
                    .canonicalize_object(reference.object_descriptor())?
                    .into();
                Ok(Object::try_from(canonical).unwrap().into_ref().into())
            }
            HandleView::Thunk(thunk) => Ok(self.canonicalize_thunk(*thunk)?.into()),
            HandleView::Encode(encode) => match encode.view() {
                EncodeView::Strict(thunk) => Ok(self.canonicalize_thunk(thunk)?.strict().into()),
                EncodeView::Shallow(thunk) => Ok(self.canonicalize_thunk(thunk)?.shallow().into()),
            },
        }
    }

    fn canonicalize_thunk(&self, thunk: Thunk) -> Result<Thunk, StorageError> {
        match thunk.view() {
            ThunkView::Identification(reference) => {
                let canonical = self.canonicalize_handle(reference.into())?;
                Ok(Ref::try_from(canonical).unwrap().identification())
            }
            ThunkView::Application(reference) => {
                let canonical = self.canonicalize_handle(reference.into())?;
                Ok(TreeRef::try_from(canonical).unwrap().application())
            }
            ThunkView::Selection(reference) => {
                let canonical = self.canonicalize_handle(reference.into())?;
                Ok(TreeRef::try_from(canonical).unwrap().selection())
            }
        }
    }
}

impl Storage for MemoryStorage {
    fn identify(&self, bytes: &[u8; HANDLE_SIZE]) -> Result<Handle, StorageError> {
        let view = Handle::parse_ref(bytes).map_err(|_| StorageError::NotFound)?;
        if !matches!(view.view(), HandleBytesView::Object(_)) {
            return Err(StorageError::UnsupportedName);
        }
        if let Name::Literal(contents) = view.name() {
            return Ok(Blob::literal(contents).unwrap().into());
        }
        if view.byte_len() == 0 && bytes == Tree::empty().as_handle().as_bytes() {
            return Ok(Tree::empty().into());
        }
        if matches!(view.name(), Name::Machine { .. }) {
            let object = unsafe { Handle::parse(*bytes) }.map_err(|_| StorageError::NotFound)?;
            return self
                .contains(object)
                .then_some(object)
                .ok_or(StorageError::NotFound);
        }
        self.inner
            .canonicals
            .with(bytes, |canonicals| {
                canonicals
                    .get_key_value(bytes)
                    .map(|(name, _)| (*name).into())
            })
            .ok_or(StorageError::NotFound)
    }

    fn canonicalize(&self, handle: Handle) -> Result<CanonicalHandle, StorageError> {
        self.canonicalize_handle(handle).map(CanonicalHandle)
    }

    fn add_blob(&self, data: &[u8]) -> Blob {
        if data.len() <= 30 {
            return Blob::literal(data).expect("literal size checked");
        }
        self.add_blob_backing(ImmutableBytes::new(data))
    }

    fn add_blob_backing(&self, data: ImmutableBytes) -> Blob {
        if data.len() <= 30 {
            return Blob::literal(&data.to_bytes()).unwrap();
        }
        let index = self.next(&self.inner.next_blob);
        let handle = unsafe {
            Blob::named(
                Name::Machine {
                    index: index as u64,
                    storage_id: self.inner.id,
                },
                data.len() as u64,
                true,
            )
        }
        .expect("blob exceeds handle length");
        self.inner.blobs.insert(index, data);
        handle
    }

    fn add_tree(&self, data: &[Handle]) -> Tree {
        let bytes = unsafe {
            core::slice::from_raw_parts(data.as_ptr().cast(), core::mem::size_of_val(data))
        };
        unsafe { self.add_tree_backing(ImmutableBytes::new(bytes)) }
    }

    unsafe fn add_tree_backing(&self, data: ImmutableBytes) -> Tree {
        if data.is_empty() {
            return Tree::empty();
        }
        let mut hash = self.inner.tree_hasher.build_hasher();
        data.for_each_chunk(|chunk| {
            for handle in chunk.as_chunks::<HANDLE_SIZE>().0 {
                hash.write(handle);
            }
        });
        let key = hash.finish();
        self.inner.tree_index.with(&key, |index| {
            let candidates = index.entry(key).or_default();
            for tree in candidates.iter() {
                let existing = self.get_tree_backing(*tree).expect("interned tree");
                if existing.len() != data.len() {
                    continue;
                }
                let mut left = existing.into_buf();
                let mut right = data.clone().into_buf();
                let mut equal = true;
                while left.has_remaining() {
                    let count = left.chunk().len().min(right.chunk().len());
                    if left.chunk()[..count] != right.chunk()[..count] {
                        equal = false;
                        break;
                    }
                    left.advance(count);
                    right.advance(count);
                }
                if equal {
                    return *tree;
                }
            }
            let (comparable, value) = tree_properties(&data);
            let tree = self.tree(TreeBacking::Ready(data), comparable, value);
            candidates.push(tree);
            tree
        })
    }

    fn get_blob(&self, name: Blob) -> Result<Box<[u8]>, StorageError> {
        if let Some(bytes) = name.literal_bytes() {
            return Ok(bytes.into());
        }
        Ok(self.get_blob_backing(name)?.to_bytes())
    }

    fn get_tree(&self, name: Tree) -> Result<Box<[Handle]>, StorageError> {
        self.get_tree_backing(name)?.read_handles()
    }

    fn get_blob_backing(&self, name: Blob) -> Result<ImmutableBytes, StorageError> {
        if let Some(bytes) = name.literal_bytes() {
            return Ok(ImmutableBytes::new(bytes));
        }
        let index = self.index(name.as_handle())?;
        let data = self
            .inner
            .blobs
            .get(index, ImmutableBytes::clone)
            .ok_or(StorageError::NotFound)?;
        if data.len() as u64 != name.byte_len() {
            return Err(StorageError::LengthMismatch);
        }
        if name.as_handle().comparability_bit() != Some(true) {
            return Err(StorageError::InvalidComparisonBit);
        }
        if name.as_handle().value_bit() != Some(true) {
            return Err(StorageError::InvalidValueBit);
        }
        Ok(data)
    }

    fn get_tree_backing(&self, name: Tree) -> Result<ImmutableBytes, StorageError> {
        if name.is_empty() && name.as_handle().same_encoding(&Tree::empty().into()) {
            return Ok(ImmutableBytes::new(&[]));
        }
        let index = self.index(name.as_handle())?;
        let (data, comparable, value) = self
            .inner
            .trees
            .get(index, Clone::clone)
            .ok_or(StorageError::NotFound)?;
        if data.len() as u64 != name.byte_len() {
            return Err(StorageError::LengthMismatch);
        }
        if name.as_handle().comparability_bit() != Some(comparable) {
            return Err(StorageError::InvalidComparisonBit);
        }
        if name.as_handle().value_bit() != Some(value) {
            return Err(StorageError::InvalidValueBit);
        }
        data.get(self, comparable, value)
    }

    fn canonicalize_blob(&self, blob: Blob) -> Result<CanonicalHandle, StorageError> {
        if blob.as_handle().comparability_bit() == Some(false) {
            return Err(StorageError::InvalidComparisonBit);
        }
        if blob.as_handle().value_bit() == Some(false) {
            return Err(StorageError::InvalidValueBit);
        }
        if let Ok(canonical) = CanonicalHandle::try_from(blob) {
            return Ok(canonical);
        }
        if let Some(canonical) = self.alias(blob.as_handle()) {
            return Ok(canonical);
        }
        let index = self.index(blob.as_handle())?;
        let bytes = self
            .inner
            .blobs
            .get(index, ImmutableBytes::clone)
            .ok_or(StorageError::NotFound)?;
        if blob.byte_len() != bytes.len() as u64 {
            return Err(StorageError::LengthMismatch);
        }
        let bytes = bytes.to_bytes();

        let canonicalized_blob = Blob::canonical(&bytes).expect("stored blob length");
        let canonical_handle =
            CanonicalHandle::try_from(canonicalized_blob).expect("canonical blob");
        self.inner.canonicals.with(&canonical_handle, |canonicals| {
            canonicals.entry(canonical_handle).or_insert(index);
        });
        self.cache_alias(blob.into(), canonical_handle);
        Ok(canonical_handle)
    }

    fn canonicalize_tree(&self, tree: Tree) -> Result<CanonicalHandle, StorageError> {
        self.canonicalize_object(tree.object())
    }
}
