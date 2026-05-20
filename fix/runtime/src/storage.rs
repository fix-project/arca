// use crate::data::{BlobData, RawData, TreeData};
use core::{marker::PhantomData, mem, ptr, slice};
use fixhandle::rawhandle::{BlobName, CanonicalHandle, Handle, LiteralHandle, TreeName};
use hashbrown::HashMap;
use kernel::prelude::*;

#[allow(unused)]
#[derive(Debug)]
struct RefCnt<T> {
    inner: T,
    count: usize,
}

#[allow(unused)]
impl<T> RefCnt<T> {
    fn new(inner: T) -> Self {
        Self { inner, count: 0 }
    }
}

pub trait FixData: From<Value> {
    fn inner(self) -> Value;
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
struct RawObjectStore {
    store: HashMap<[u8; 32], Value>,
}

impl Default for RawObjectStore {
    fn default() -> Self {
        Self {
            store: HashMap::new(),
        }
    }
}

impl RawObjectStore {
    fn new() -> Self {
        Self::default()
    }

    fn create_raw(&mut self, data: Blob) {
        let hash = blake3::hash(data.as_ref());
        let bytes = hash.as_bytes();
        let canonical = CanonicalHandle::new(*bytes, 0);
        self.store.insert(canonical.hash(), data.into());
    }

    fn create<T: FixData + AsRef<[u8]>>(&mut self, data: T) -> CanonicalHandle {
        let hash = blake3::hash(data.as_ref());
        let hash = *hash.as_bytes();
        let handle = CanonicalHandle::new(hash, data.len());
        self.store.insert(handle.hash(), data.inner());
        handle
    }

    fn get(&self, key: &[u8; 32]) -> Value {
        self.store
            .get(key)
            .expect("Failed to retrieve data")
            .clone()
    }
}

pub trait Storage<FixBlobData: FixData + AsRef<[u8]>, FixTreeData: FixData + AsRef<[u8]>> {
    fn create_blob(&mut self, data: FixBlobData) -> BlobName;
    fn create_tree(&mut self, data: FixTreeData) -> TreeName;
    fn get_blob(&self, handle: &BlobName) -> FixBlobData;
    fn get_tree(&self, handle: &TreeName) -> FixTreeData;
}

#[derive(Debug)]
pub struct ObjectStore<FixBlobData: FixData + AsRef<[u8]>, FixTreeData: FixData + AsRef<[u8]>> {
    store: RawObjectStore,
    _blob: PhantomData<FixBlobData>,
    _tree: PhantomData<FixTreeData>,
}

impl<FixBlobData: FixData + AsRef<[u8]>, FixTreeData: FixData + AsRef<[u8]>>
    Storage<FixBlobData, FixTreeData> for ObjectStore<FixBlobData, FixTreeData>
{
    fn create_blob(&mut self, data: FixBlobData) -> BlobName {
        if data.len() < 30 {
            BlobName::Literal(LiteralHandle::new(data.as_ref()))
        } else {
            let canonical = self.store.create(data);
            BlobName::Blob(Handle::CanonicalHandle(canonical))
        }
    }

    fn create_tree(&mut self, data: FixTreeData) -> TreeName {
        let canonical = self.store.create(data);
        TreeName::NotTag(Handle::CanonicalHandle(canonical))
    }

    fn get_blob(&self, handle: &BlobName) -> FixBlobData {
        match handle {
            BlobName::Blob(h) => match h {
                Handle::VirtualHandle(_) => todo!(),
                Handle::CanonicalHandle(c) => self.store.get(&c.hash()).into(),
                Handle::PhysicalHandle(_) => todo!(),
            },
            BlobName::Literal(l) => {
                let data: Value = Blob::new(l.content()).into();
                data.into()
            }
        }
    }

    fn get_tree(&self, handle: &TreeName) -> FixTreeData {
        match handle {
            TreeName::NotTag(t) | TreeName::Tag(t) => match t {
                Handle::VirtualHandle(_) => todo!(),
                Handle::CanonicalHandle(c) => self.store.get(&c.hash()).into(),
                Handle::PhysicalHandle(_) => todo!(),
            },
        }
    }
}

impl<FixBlobData: FixData + AsRef<[u8]>, FixTreeData: FixData + AsRef<[u8]>> Default
    for ObjectStore<FixBlobData, FixTreeData>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<FixBlobData: FixData + AsRef<[u8]>, FixTreeData: FixData + AsRef<[u8]>>
    ObjectStore<FixBlobData, FixTreeData>
{
    fn into(v: Value) -> Box<[u8]> {
        match v {
            Value::Blob(b) => b.into_inner().into_inner(),
            _ => todo!(),
        }
    }

    pub fn new() -> Self {
        Self {
            store: RawObjectStore::default(),
            _blob: Default::default(),
            _tree: Default::default(),
        }
    }

    pub fn into_raw_parts<T>(input: Box<[T]>) -> (usize, usize) {
        let data = Box::into_raw(input);
        let len = ptr::metadata(data);
        let offset = if len > 0 {
            BuddyAllocator.to_offset(data)
        } else {
            0
        };
        (offset, len)
    }

    pub fn from_raw_parts<T>(offset: usize, len: usize) -> Box<[T]> {
        if len == 0 {
            Vec::with_capacity(0).into_boxed_slice()
        } else {
            let data = BuddyAllocator.from_offset(offset);
            unsafe {
                let slice = slice::from_raw_parts_mut(data, len);
                Box::from_raw(slice)
            }
        }
    }

    pub fn load(&mut self, input: Box<[(usize, usize)]>) {
        for (offset, len) in input.into_iter() {
            let x = Self::from_raw_parts(offset, len);
            self.store.create_raw(Blob::new(x));
        }
        log::info!("Loaded {:?} objects", self.store.store.len());
    }

    pub fn unload(&mut self) -> Box<[(usize, usize)]> {
        let mut res = Vec::with_capacity(2 * 8 * self.store.store.len());

        let v = mem::take(&mut self.store.store);

        for (_key, value) in v.into_iter() {
            let value: Box<[u8]> = Self::into(value);
            res.push(Self::into_raw_parts(value));
        }

        res.into_boxed_slice()
    }
}
