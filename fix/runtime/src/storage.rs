use crate::data::{BlobData, RawData, TreeData};
use fixhandle::rawhandle::{BlobName, Handle, PhysicalHandle, TreeName};
use kernel::prelude::*;

#[derive(Debug)]
struct RefCnt<T> {
    inner: T,
    count: usize,
}

impl<T> RefCnt<T> {
    fn new(inner: T) -> Self {
        Self { inner, count: 0 }
    }
}

#[derive(Debug)]
struct RawObjectStore<Data: Clone> {
    table: Vec<RefCnt<Data>>,
}

impl<Data: Clone> Default for RawObjectStore<Data> {
    fn default() -> Self {
        Self { table: vec![] }
    }
}

impl<Data: Clone> RawObjectStore<Data> {
    fn new() -> Self {
        Self::default()
    }

    fn create(&mut self, data: Data) -> usize {
        let idx = self.table.len();
        self.table.push(RefCnt::new(data));
        idx
    }

    fn get(&self, idx: usize) -> Data {
        self.table[idx].inner.clone()
    }
}

pub trait Storage {
    fn create_blob(&mut self, data: BlobData) -> BlobName;
    fn create_tree(&mut self, data: TreeData) -> TreeName;
    fn get_blob(&self, handle: &BlobName) -> BlobData;
    fn get_tree(&self, handle: &TreeName) -> TreeData;
}

#[derive(Default, Debug)]
pub struct ObjectStore {
    store: RawObjectStore<RawData>,
}

impl ObjectStore {
    fn new() -> Self {
        Self::default()
    }
}

impl Storage for ObjectStore {
    fn create_blob(&mut self, data: BlobData) -> BlobName {
        let len = data.len();
        let local_id = self.store.create(data.into());
        BlobName::Blob(Handle::PhysicalHandle(PhysicalHandle::new(local_id, len)))
    }

    fn create_tree(&mut self, data: TreeData) -> TreeName {
        let len = data.len();
        let local_id = self.store.create(data.into());
        TreeName::NotTag(Handle::PhysicalHandle(PhysicalHandle::new(local_id, len)))
    }

    fn get_blob(&self, handle: &BlobName) -> BlobData {
        match handle {
            BlobName::Blob(h) => match h {
                Handle::VirtualHandle(_) => todo!(),
                Handle::PhysicalHandle(physical_handle) => {
                    self.store.get(physical_handle.local_id()).into()
                }
            },
        }
    }

    fn get_tree(&self, handle: &TreeName) -> TreeData {
        match handle {
            TreeName::NotTag(t) | TreeName::Tag(t) => match t {
                Handle::VirtualHandle(_) => todo!(),
                Handle::PhysicalHandle(physical_handle) => {
                    self.store.get(physical_handle.local_id()).into()
                }
            },
        }
    }
}
