use crate::{
    fixruntime::{CouponHelper, DeterministicEquivRuntime, RuntimeError},
    storageruntime::StorageRuntime,
    vmmruntime::VmmRuntime,
};
use common::bitpack::BitPack;
use fixhandle::rawhandle::{BlobName, FixHandle, Object};
use std::{collections::HashMap, sync::Arc};

pub struct HybridRuntime {
    vmm_runtime: VmmRuntime,
    storage_runtime: StorageRuntime,
    // Packed FixHandle to packed CanonicalHandle if flushed or packed FixHandle if literal
    flushed: HashMap<[u8; 32], [u8; 32]>,
    store: Vec<FixHandle>,
}

impl HybridRuntime {
    pub fn new(smp: usize, cid: usize, bin: Arc<[u8]>) -> Self {
        Self {
            vmm_runtime: VmmRuntime::new(smp, cid, bin),
            storage_runtime: StorageRuntime::new(),
            flushed: HashMap::new(),
            store: Vec::new(),
        }
    }

    fn flush_handle(&mut self, handle: FixHandle) -> Result<[u8; 32], RuntimeError> {
        let packed_handle = handle.pack();

        if let Some(flushed_handle) = self.flushed.get(&packed_handle) {
            return Ok(*flushed_handle);
        }

        let canonical_handle = match handle {
            FixHandle::Object(Object::BlobObj(blob_name)) => match blob_name {
                // Store packed handle for literals
                BlobName::Literal(_) => packed_handle,
                // Write non-literals to storage
                BlobName::Blob(_) => {
                    let blob_handle = FixHandle::Object(Object::BlobObj(blob_name));
                    let blob = self.vmm_runtime.get_blob(&blob_handle)?;
                    self.storage_runtime.create_blob(blob).pack()
                }
            },
            FixHandle::Object(Object::TreeObj(_)) => {
                let tree = self.vmm_runtime.get_tree(&handle)?;
                let mut children = Vec::with_capacity(Self::get_tree_len(tree));

                for i in 0..Self::get_tree_len(tree) {
                    let child = Self::get_tree_entry(tree, i);
                    children.push(child);
                }

                let mut bytes = Vec::with_capacity(tree.len());
                for child in children {
                    let flushed = self.flush_handle(child)?;
                    bytes.extend_from_slice(&flushed);
                }

                self.storage_runtime.create_tree(&bytes).pack()
            }
            _ => todo!(),
        };

        self.flushed.insert(packed_handle, canonical_handle);
        Ok(canonical_handle)
    }

    fn flush(&mut self) {
        for handle in self.store.clone() {
            let _ = self.flush_handle(handle);
        }
    }
}

impl Drop for HybridRuntime {
    fn drop(&mut self) {
        self.flush();
    }
}

impl CouponHelper for HybridRuntime {}

impl DeterministicEquivRuntime for HybridRuntime {
    type BlobData<'a> = &'a [u8];
    type TreeData<'a> = &'a [u8];
    type Handle = FixHandle;
    type Error = RuntimeError;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        let bytes = data.to_le_bytes();
        self.create_blob(&bytes)
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        let bytes = data.to_le_bytes();
        self.create_blob(&bytes)
    }

    fn create_blob(&mut self, data: &[u8]) -> Self::Handle {
        let handle = self.vmm_runtime.create_blob(data);
        self.store.push(handle);
        handle
    }

    fn create_tree(&mut self, data: &[u8]) -> Self::Handle {
        let handle = self.vmm_runtime.create_tree(data);
        self.store.push(handle);
        handle
    }

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error> {
        self.vmm_runtime.get_blob(handle)
    }

    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error> {
        self.vmm_runtime.get_tree(handle)
    }

    fn apply(&mut self, handle: &Self::Handle) -> Result<Self::Handle, RuntimeError> {
        self.vmm_runtime.apply(handle)
    }
}
