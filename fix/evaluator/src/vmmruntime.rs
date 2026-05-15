use core::slice;
use std::sync::Arc;
use std::{mem, ptr};

use common::BuddyAllocator;
use common::bitpack::BitPack;
use fixhandle::rawhandle::{
    BlobName, FixHandle, Handle, LiteralHandle, Object, PhysicalHandle, TreeName,
};
use vmm::runtime::Runtime;

use crate::fixruntime::{CouponHelper, DeterministicEquivRuntime, Operator, RuntimeError};
use crate::vmcommon::{CouponTrades, FixOp};

pub struct VmmRuntime {
    runtime: Runtime,
    store: Vec<Box<[u8], BuddyAllocator>>,
    executed: bool,
}

impl VmmRuntime {
    pub fn new(smp: usize, cid: usize, bin: Arc<[u8]>) -> Self {
        Self {
            runtime: Runtime::new(cid, smp, 1 << 34, bin),
            store: Vec::new(),
            executed: false,
        }
    }

    fn create(&mut self, data: Box<[u8], BuddyAllocator>) -> usize {
        let idx = self.store.len();
        self.store.push(data);
        idx
    }

    fn get(&self, idx: usize) -> &[u8] {
        &self.store[idx]
    }

    fn get_by_handle(&self, handle: Handle) -> &[u8] {
        match handle {
            Handle::VirtualHandle(_) | Handle::CanonicalHandle(_) => todo!(),
            Handle::PhysicalHandle(physical_handle) => self.get(physical_handle.local_id()),
        }
    }
}

impl DeterministicEquivRuntime for VmmRuntime {
    type BlobData<'a> = &'a [u8];
    type TreeData<'a> = &'a [u8];
    type Handle = FixHandle;
    type Error = RuntimeError;

    fn create_blob_i32(&mut self, data: u32) -> FixHandle {
        let bytes = data.to_le_bytes();
        self.create_blob(&bytes)
    }

    fn create_blob_i64(&mut self, data: u64) -> FixHandle {
        let bytes = data.to_le_bytes();
        self.create_blob(&bytes)
    }

    fn create_blob(&mut self, data: Self::BlobData<'_>) -> FixHandle {
        let len = data.len();
        if len <= 30 {
            let literal = LiteralHandle::new(data);
            Object::from(BlobName::Literal(literal)).into()
        } else {
            let local_id = self.create(data.to_vec_in(BuddyAllocator).into_boxed_slice());
            let blob = BlobName::Blob(Handle::PhysicalHandle(PhysicalHandle::new(local_id, len)));
            Object::from(blob).into()
        }
    }

    fn create_tree(&mut self, data: Self::TreeData<'_>) -> FixHandle {
        let len = data.len() / 32;
        let local_id = self.create(data.to_vec_in(BuddyAllocator).into_boxed_slice());
        let tree = TreeName::NotTag(Handle::PhysicalHandle(PhysicalHandle::new(local_id, len)));
        Object::from(tree).into()
    }

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error> {
        let b = handle
            .try_unwrap_object_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?
            .try_unwrap_blob_obj_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?;

        match b {
            BlobName::Blob(h) => Ok(self.get_by_handle(*h)),
            BlobName::Literal(l) => Ok(l.content()),
        }
    }

    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error> {
        let t = handle
            .try_unwrap_object_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?
            .try_unwrap_tree_obj_ref()
            .map_err(|_| RuntimeError::TypeMismatch)?;
        match t {
            TreeName::NotTag(tree) | TreeName::Tag(tree) => Ok(self.get_by_handle(*tree)),
        }
    }
}

impl VmmRuntime {
    fn into_raw_parts<T>(input: Box<[T], BuddyAllocator>) -> (usize, usize) {
        let (data, _) = Box::into_raw_with_allocator(input);
        let len = ptr::metadata(data);
        let offset = if len > 0 {
            BuddyAllocator.to_offset(data)
        } else {
            0
        };
        (offset, len)
    }

    fn from_raw_parts<T>(offset: usize, len: usize) -> Box<[T], BuddyAllocator> {
        if len == 0 {
            Vec::with_capacity_in(0, BuddyAllocator).into_boxed_slice()
        } else {
            let data = BuddyAllocator.from_offset(offset);
            unsafe {
                let slice = slice::from_raw_parts_mut(data, len);
                Box::from_raw_in(slice, BuddyAllocator)
            }
        }
    }

    fn load(&mut self) -> Box<[(usize, usize)], BuddyAllocator> {
        let mut res = Vec::with_capacity_in(2 * 8 * self.store.len(), BuddyAllocator);

        let v = mem::take(&mut self.store);

        for entry in v.into_iter() {
            res.push(Self::into_raw_parts(entry));
        }

        res.into_boxed_slice()
    }

    fn unload(&mut self, input: Box<[(usize, usize)], BuddyAllocator>) {
        for (offset, len) in input.into_iter() {
            self.store.push(Self::from_raw_parts(offset, len));
        }
    }

    fn run(
        &mut self,
        opcode: FixOp,
        coupon_trade: Option<CouponTrades>,
        handle: FixHandle,
    ) -> FixHandle {
        if self.executed {
            panic!("Multiple apply/eval/trade not supported yet.")
        }

        self.executed = true;

        let handle_scratch: Box<[u8], _> = Box::new_in(handle.pack(), BuddyAllocator);
        let output_store: Box<[usize], _> = Box::new_in([0; 2], BuddyAllocator);

        let (handle_scratch_offset, handle_scratch_len) = Self::into_raw_parts(handle_scratch);
        let (store_offset, store_len) = Self::into_raw_parts(self.load());
        let (output_store_offset, output_store_len) = Self::into_raw_parts(output_store);

        let args = [
            opcode as usize,
            coupon_trade.map_or_default(|t| t as usize),
            handle_scratch_offset,
            handle_scratch_len,
            store_offset,
            store_len,
            output_store_offset,
            output_store_len,
        ];
        self.runtime.run(&args);

        let output_store: Box<[usize], BuddyAllocator> =
            Self::from_raw_parts(output_store_offset, output_store_len);
        let output_store_slice: &[usize; 2] = output_store
            .as_ref()
            .try_into()
            .expect("Failed to convert output store back");
        self.unload(Self::from_raw_parts(
            output_store_slice[0],
            output_store_slice[1],
        ));

        let handle_scratch: Box<[u8], BuddyAllocator> =
            Self::from_raw_parts(handle_scratch_offset, handle_scratch_len);
        let handle_slice: &[u8; 32] = handle_scratch
            .as_ref()
            .try_into()
            .expect("Failed to convert handle_slice back");
        FixHandle::unpack(*handle_slice)
    }
}

impl CouponHelper for VmmRuntime {}

impl Operator for VmmRuntime {
    fn eval(&mut self, handle: FixHandle) -> FixHandle {
        self.run(FixOp::Eval, None, handle)
    }

    fn trade(
        &mut self,
        trade_type: CouponTrades,
        coupons: FixHandle,
        lhs: FixHandle,
        rhs: FixHandle,
    ) -> FixHandle {
        let mut scratch = Vec::with_capacity(3 * 32);
        scratch.extend_from_slice(&coupons.pack());
        scratch.extend_from_slice(&lhs.pack());
        scratch.extend_from_slice(&rhs.pack());
        let scratch = self.create_tree(scratch.as_slice());

        self.run(FixOp::Trade, Some(trade_type), scratch)
    }

    fn apply(&mut self, handle: FixHandle) -> FixHandle {
        self.run(FixOp::Apply, None, handle)
    }
}
