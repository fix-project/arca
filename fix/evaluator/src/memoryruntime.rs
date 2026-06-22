use crate::fixruntime::{DeterministicEquivRuntime, RuntimeError};
use fixhandle::rawhandle::{
    BlobName, FixHandle, Handle, LiteralHandle, Object, PhysicalHandle, TreeName,
};

pub struct MemoryRuntime {
    store: Vec<Box<[u8]>>,
}

impl Default for MemoryRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryRuntime {
    pub fn new() -> Self {
        Self { store: Vec::new() }
    }

    fn create(&mut self, data: &[u8]) -> usize {
        let index = self.store.len();
        self.store.push(Box::from(data));
        index
    }

    fn get(&self, idx: usize) -> &[u8] {
        &self.store[idx]
    }

    pub fn get_by_handle(&self, handle: Handle) -> &[u8] {
        match handle {
            Handle::VirtualHandle(_) | Handle::CanonicalHandle(_) => todo!(),
            Handle::PhysicalHandle(physical_handle) => self.get(physical_handle.local_id()),
        }
    }
}

impl DeterministicEquivRuntime for MemoryRuntime {
    type BlobData<'a> = &'a [u8];
    type TreeData<'a> = &'a [u8];
    type Handle = FixHandle;
    type Error = RuntimeError;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        self.create_blob(&data.to_le_bytes())
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        self.create_blob(&data.to_le_bytes())
    }

    fn create_blob(&mut self, data: &[u8]) -> Self::Handle {
        if data.len() <= 30 {
            let literal = LiteralHandle::new(data);
            Object::from(BlobName::Literal(literal)).into()
        } else {
            let blob = BlobName::Blob(Handle::PhysicalHandle(PhysicalHandle::new(
                self.create(data),
                data.len(),
            )));
            Object::from(blob).into()
        }
    }

    fn create_tree(&mut self, data: &[u8]) -> Self::Handle {
        let tree = TreeName::NotTag(Handle::PhysicalHandle(PhysicalHandle::new(
            self.create(data),
            data.len() / 32,
        )));
        Object::from(tree).into()
    }

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error> {
        let blob = handle
            .try_unwrap_object_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?
            .try_unwrap_blob_obj_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?;

        match blob {
            BlobName::Blob(h) => Ok(self.get_by_handle(*h)),
            BlobName::Literal(literal) => Ok(literal.content()),
        }
    }

    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error> {
        let tree = handle
            .try_unwrap_object_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?
            .try_unwrap_tree_obj_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?;

        match tree {
            TreeName::NotTag(h) | TreeName::Tag(h) => Ok(self.get_by_handle(*h)),
        }
    }
}
