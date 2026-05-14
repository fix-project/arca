// use crate::data::{BlobData, RawData, TreeData};
use core::{mem, ptr, slice};
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
    fn create_blob(&mut self, data: Blob) -> BlobName;
    fn create_tree(&mut self, data: Blob) -> TreeName;
    fn get_blob(&self, handle: &BlobName) -> Blob;
    fn get_tree(&self, handle: &TreeName) -> Blob;
}

#[derive(Default, Debug)]
pub struct ObjectStore {
    store: RawObjectStore<Value>,
}

impl ObjectStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for ObjectStore {
    fn create_blob(&mut self, data: Blob) -> BlobName {
        let len = data.len();
        let local_id = self.store.create(data.into());
        BlobName::Blob(Handle::PhysicalHandle(PhysicalHandle::new(local_id, len)))
    }

    fn create_tree(&mut self, data: Blob) -> TreeName {
        let len = data.len() / 32;
        let local_id = self.store.create(data.into());
        TreeName::NotTag(Handle::PhysicalHandle(PhysicalHandle::new(local_id, len)))
    }

    fn get_blob(&self, handle: &BlobName) -> Blob {
        match handle {
            BlobName::Blob(h) => match h {
                Handle::VirtualHandle(_) => todo!(),
                Handle::CanonicalHandle(_) => todo!(),
                Handle::PhysicalHandle(physical_handle) => self
                    .store
                    .get(physical_handle.local_id())
                    .try_into()
                    .unwrap(),
            },
            BlobName::Literal(h) => Blob::new(h.content()),
        }
    }

    fn get_tree(&self, handle: &TreeName) -> Blob {
        match handle {
            TreeName::NotTag(t) | TreeName::Tag(t) => match t {
                Handle::VirtualHandle(_) => todo!(),
                Handle::CanonicalHandle(_) => todo!(),
                Handle::PhysicalHandle(physical_handle) => self
                    .store
                    .get(physical_handle.local_id())
                    .try_into()
                    .unwrap(),
            },
        }
    }
}

impl ObjectStore {
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
            self.store.create(Blob::new(x).into());
        }
        log::info!("Loaded {:?} objects", self.store.table.len());
    }

    pub fn unload(&mut self) -> Box<[(usize, usize)]> {
        let mut res = Vec::with_capacity(2 * 8 * self.store.table.len());

        let v = mem::take(&mut self.store.table);

        for entry in v.into_iter() {
            let blob: Blob = entry.inner.try_into().unwrap();
            let blob = blob.into_inner().into_inner();
            res.push(Self::into_raw_parts(blob));
        }

        res.into_boxed_slice()
    }
}
