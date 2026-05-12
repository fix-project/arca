#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::{
    bottom::FixShellBottom,
    // data::{BlobData, TreeData},
    runtime::{CouponHelper, CouponTrades, DeterministicEquivRuntime, Executor},
    storage::{ObjectStore, Storage},
};
use bytemuck::bytes_of;
use common::bitpack::BitPack;
use derive_more::TryUnwrapError;
use fixhandle::rawhandle::{FixHandle, Object, TreeName};
use kernel::types::{Blob, Tuple};

#[derive(Debug)]
pub enum Error {
    OOB,
    TypeMismatch,
}

impl<T> From<TryUnwrapError<T>> for Error {
    fn from(_value: TryUnwrapError<T>) -> Self {
        Error::TypeMismatch
    }
}

#[derive(Debug)]
pub struct FixRuntime<'a> {
    store: &'a mut ObjectStore,
    coupon: FixHandle,
}

impl<'a> FixRuntime<'a> {
    pub fn new(store: &'a mut ObjectStore, coupon: &[u8]) -> Self {
        let coupon = Object::from(store.create_blob(coupon.into())).into();
        Self { store, coupon }
    }
}

impl<'a> DeterministicEquivRuntime for FixRuntime<'a> {
    type BlobData = Blob;
    type TreeData = Tuple;
    type Handle = FixHandle;
    type Error = Error;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        let buf = bytes_of(&data);
        Object::from(self.store.create_blob(buf.into())).into()
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        let buf = bytes_of(&data);
        Object::from(self.store.create_blob(buf.into())).into()
    }

    fn create_blob(&mut self, data: Blob) -> Self::Handle {
        Object::from(self.store.create_blob(data)).into()
    }

    fn create_tree(&mut self, data: Tuple) -> Self::Handle {
        Object::from(self.store.create_tree(data)).into()
    }

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error> {
        let b = handle
            .try_unwrap_object_ref()
            .map_err(Error::from)?
            .try_unwrap_blob_obj_ref()
            .map_err(|_| Error::TypeMismatch)?;
        Ok(self.store.get_blob(b))
    }

    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error> {
        let t = handle
            .try_unwrap_object_ref()
            .map_err(Error::from)?
            .try_unwrap_tree_obj_ref()
            .map_err(Error::from)?;
        Ok(self.store.get_tree(t))
    }

    fn is_blob(handle: &Self::Handle) -> bool {
        handle
            .try_unwrap_object_ref()
            .map_err(Error::from)
            .and_then(|h| h.try_unwrap_blob_obj_ref().map_err(Error::from))
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .map_err(Error::from)
                .and_then(|h| h.try_unwrap_blob_ref_ref().map_err(Error::from))
                .is_ok()
    }

    fn is_tree(handle: &Self::Handle) -> bool {
        handle
            .try_unwrap_object_ref()
            .map_err(Error::from)
            .and_then(|h| h.try_unwrap_tree_obj_ref().map_err(Error::from))
            .is_ok()
            || handle
                .try_unwrap_ref_ref()
                .map_err(Error::from)
                .and_then(|h| h.try_unwrap_tree_ref_ref().map_err(Error::from))
                .is_ok()
    }

    fn is_equal(lhs: &Self::Handle, rhs: &Self::Handle) -> bool {
        lhs.pack() == rhs.pack()
    }
}

impl<'a> Executor for FixRuntime<'a> {
    fn execute(&mut self, combination: &FixHandle) -> FixHandle {
        let mut bottom = FixShellBottom { parent: self };
        let res = bottom.execute(combination);
        let mut apply_coupon: Tuple = Tuple::new(4);
        apply_coupon.set(0, Blob::new(self.coupon.pack()));
        apply_coupon.set(1, Blob::new(self.create_blob_i32(2).pack()));
        apply_coupon.set(2, Blob::new(combination.pack()));
        apply_coupon.set(3, Blob::new(res.pack()));
        Object::from(TreeName::Tag(
            self.create_tree(apply_coupon)
                .unwrap_object()
                .unwrap_tree_obj()
                .unwrap_not_tag(),
        ))
        .into()
    }
}

impl<'a> CouponHelper for FixRuntime<'a> {
    fn read_blob(blob: &Self::BlobData, offset: usize, buf: &mut [u8]) -> usize {
        blob.read(offset, buf)
    }

    fn get_tree_entry(tree: &Self::TreeData, offset: usize) -> Self::Handle {
        let mut scratch: [u8; 32] = [0; 32];
        let entry: Blob = tree.get(offset).try_into().expect("tree entry not a blob");
        entry.read(0, &mut scratch);
        FixHandle::unpack(scratch)
    }

    fn trade(
        &mut self,
        trade_type: CouponTrades,
        coupons: FixHandle,
        lhs: FixHandle,
        rhs: FixHandle,
    ) -> FixHandle {
        let mut combination = Tuple::new(6);
        combination.set(0, Blob::new(self.create_blob_i32(0).pack()));
        combination.set(1, Blob::new(self.coupon.pack()));
        combination.set(2, Blob::new(self.create_blob_i32(trade_type as u32).pack()));
        combination.set(3, Blob::new(coupons.pack()));
        combination.set(4, Blob::new(lhs.pack()));
        combination.set(5, Blob::new(rhs.pack()));

        let combination = self.create_tree(combination);
        let mut bottom = FixShellBottom { parent: self };
        bottom.execute(&combination)
    }
}
