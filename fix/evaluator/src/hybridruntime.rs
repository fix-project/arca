use crate::{
    fixruntime::{CouponHelper, DeterministicEquivRuntime, Operator, RuntimeError},
    storageruntime::StorageRuntime,
    vmmruntime::VmmRuntime,
    vmcommon::CouponTrades,
};
use common::bitpack::BitPack;
use fixhandle::rawhandle::{
    BlobName, TreeName, Encode, FixHandle, Object, Thunk, create_application_thunk, create_strict_encode
};
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

    pub fn flush_handle(&mut self, handle: FixHandle) -> Result<FixHandle, RuntimeError> {
        let packed_handle = handle.pack();

        if let Some(flushed_handle) = self.flushed.get(&packed_handle) {
            return Ok(FixHandle::unpack(*flushed_handle));
        }

        let canonical_handle = match handle {
            FixHandle::Object(Object::BlobObj(blob_name)) => match blob_name {
                // Store packed handle for literals
                BlobName::Literal(_) => handle,
                // Write non-literals to storage
                BlobName::Blob(_) => {
                    let blob_handle = FixHandle::Object(Object::BlobObj(blob_name));
                    let blob = self.vmm_runtime.get_blob(&blob_handle)?;
                    self.storage_runtime.create_blob(blob)
                }
            },
            FixHandle::Object(Object::TreeObj(in_treename)) => {
                let tree = self.vmm_runtime.get_tree(&handle)?;
                let mut children = Vec::with_capacity(Self::get_tree_len(tree));
                for i in 0..Self::get_tree_len(tree) {
                    let child = Self::get_tree_entry(tree, i);
                    children.push(child);
                }
                let mut bytes = Vec::with_capacity(tree.len());
                for child in children {
                    let flushed = self.flush_handle(child)?;
                    bytes.extend_from_slice(&flushed.pack());
                }

                let treename = self.storage_runtime.create_tree(&bytes).unwrap_object().unwrap_tree_obj().unwrap_not_tag();
                let treename = match in_treename {
                    TreeName::Tag(_) => TreeName::Tag(treename),
                    TreeName::NotTag(_) => TreeName::NotTag(treename)
                };
                FixHandle::Object(Object::TreeObj(treename))
            }
            FixHandle::Encode(Encode::Strict(tree)) => {
                let inner = self.flush_handle(FixHandle::Thunk(tree))?;
                create_strict_encode(&inner)
                    .expect("strict encode flush failed")
            }
            FixHandle::Encode(Encode::Shallow(_tree)) => todo!(""),
            FixHandle::Thunk(Thunk::Application(tree)) => {
                let tree_handle = FixHandle::Object(Object::from(tree));
                let flushed_tree = self.flush_handle(tree_handle)?;
                create_application_thunk(&flushed_tree)
                    .expect("application thunk flush failed")
            }
            FixHandle::Thunk(_) => todo!(""),
            FixHandle::Ref(_) => todo!(""),
        };

        self.flushed.insert(packed_handle, canonical_handle.pack());
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
        self.vmm_runtime.create_blob(data)
    }

    fn create_tree(&mut self, data: &[u8]) -> Self::Handle {
        self.vmm_runtime.create_tree(data)
    }

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error> {
        self.vmm_runtime.get_blob(handle)
    }

    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error> {
        self.vmm_runtime.get_tree(handle)
    }
}

impl CouponHelper for HybridRuntime {}

impl Operator for HybridRuntime {
    fn apply(&mut self, handle: FixHandle) -> FixHandle {
        self.vmm_runtime.apply(handle)
    }

    fn eval(&mut self, handle: FixHandle) -> FixHandle {
        self.vmm_runtime.eval(handle)
    }

    fn trade(
            &mut self,
            _trade_type: CouponTrades,
            _coupons: FixHandle,
            _lhs: FixHandle,
            _rhs: FixHandle,
        ) -> FixHandle {
        todo!()
    }
}
