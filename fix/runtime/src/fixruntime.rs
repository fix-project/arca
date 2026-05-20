#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]

use crate::{
    bottom::FixShellBottom,
    common::CouponTrades,
    // data::{BlobData, TreeData},
    runtime::{DeterministicEquivRuntime, Executor},
    storage::{FixData, ObjectStore, Storage},
};
use bytemuck::bytes_of;
use common::bitpack::BitPack;
use derive_more::TryUnwrapError;
use fixhandle::rawhandle::{FixHandle, Object, TreeName};
use kernel::prelude::*;
use kernel::types::Blob;

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

#[derive(Debug, Clone)]
pub struct FixBlobData {
    inner: Blob,
}

impl FixData for FixBlobData {
    fn inner(self) -> Value {
        self.inner.into()
    }

    fn len(&self) -> u64 {
        self.inner.len() as u64
    }
}

impl From<Value> for FixBlobData {
    fn from(value: Value) -> Self {
        match value {
            Value::Blob(b) => FixBlobData { inner: b },
            _ => todo!(),
        }
    }
}

impl AsRef<[u8]> for FixBlobData {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl FixBlobData {
    pub fn new(inner: Blob) -> Self {
        Self { inner }
    }

    pub fn read(&self, offset: usize, buf: &mut [u8]) -> usize {
        self.inner.read(offset, buf)
    }
}

#[derive(Debug, Clone)]
pub struct FixTreeData {
    inner: Blob,
}

impl FixData for FixTreeData {
    fn inner(self) -> Value {
        self.inner.into()
    }

    fn len(&self) -> u64 {
        self.inner.len() as u64 / 32
    }
}

impl From<Value> for FixTreeData {
    fn from(value: Value) -> Self {
        match value {
            Value::Blob(b) => FixTreeData { inner: b },
            _ => todo!(),
        }
    }
}

impl AsRef<[u8]> for FixTreeData {
    fn as_ref(&self) -> &[u8] {
        self.inner.as_ref()
    }
}

impl FixTreeData {
    pub fn new(inner: Blob) -> Self {
        Self { inner }
    }

    pub fn get(&self, offset: usize) -> FixHandle {
        let mut scratch: [u8; 32] = [0; 32];
        self.inner.read(offset * 32, &mut scratch);
        FixHandle::unpack(scratch)
    }
}

impl From<FixBlobData> for Blob {
    fn from(val: FixBlobData) -> Self {
        val.inner
    }
}

impl From<FixTreeData> for Blob {
    fn from(val: FixTreeData) -> Self {
        val.inner
    }
}

#[derive(Debug)]
pub struct FixRuntime<'a> {
    store: &'a mut ObjectStore<FixBlobData, FixTreeData>,
    coupon: FixHandle,
}

impl<'a> FixRuntime<'a> {
    pub fn new(store: &'a mut ObjectStore<FixBlobData, FixTreeData>, coupon: &[u8]) -> Self {
        let coupon = Object::from(store.create_blob(FixBlobData::new(coupon.into()))).into();
        Self { store, coupon }
    }
}

impl<'a> DeterministicEquivRuntime for FixRuntime<'a> {
    type BlobData = FixBlobData;
    type TreeData = FixTreeData;
    type Handle = FixHandle;
    type Error = Error;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle {
        let buf = bytes_of(&data);
        let blob = Blob::new(buf);
        Object::from(self.store.create_blob(FixBlobData::new(blob))).into()
    }

    fn create_blob_i64(&mut self, data: u64) -> Self::Handle {
        let buf = bytes_of(&data);
        let blob = Blob::new(buf);
        Object::from(self.store.create_blob(FixBlobData::new(blob))).into()
    }

    fn create_blob(&mut self, data: FixBlobData) -> Self::Handle {
        Object::from(self.store.create_blob(data)).into()
    }

    fn create_tree(&mut self, data: FixTreeData) -> Self::Handle {
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
        let tree = self.store.get_tree(t);
        Ok(tree)
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

        let mut apply_coupon = Vec::with_capacity(32 * 4);
        apply_coupon.extend_from_slice(&self.coupon.pack());
        apply_coupon.extend_from_slice(&self.create_blob_i32(2).pack());
        apply_coupon.extend_from_slice(&combination.pack());
        apply_coupon.extend_from_slice(&res.pack());

        Object::from(TreeName::Tag(
            self.create_tree(FixTreeData::new(Blob::new(apply_coupon)))
                .unwrap_object()
                .unwrap_tree_obj()
                .unwrap_not_tag(),
        ))
        .into()
    }
}

impl FixRuntime<'_> {
    pub fn trade(
        &mut self,
        trade_type: CouponTrades,
        coupons: FixHandle,
        lhs: FixHandle,
        rhs: FixHandle,
    ) -> FixHandle {
        let mut combination = Vec::with_capacity(32 * 5);
        combination.extend_from_slice(&self.coupon.pack());
        combination.extend_from_slice(&self.create_blob_i32(trade_type as u32).pack());
        combination.extend_from_slice(&coupons.pack());
        combination.extend_from_slice(&lhs.pack());
        combination.extend_from_slice(&rhs.pack());

        let combination = self.create_tree(FixTreeData::new(Blob::new(combination)));
        let mut bottom = FixShellBottom { parent: self };
        bottom.execute(&combination)
    }

    fn coupon_type(&mut self, handle: &FixHandle) -> &'static str {
        let mut arr = [0u8; 4];
        let blob = self.get_blob(handle).expect("Coupon type not a blob");
        blob.read(0, &mut arr);
        let num = u32::from_le_bytes(arr);
        match num {
            0 => "Eq",
            1 => "Eval",
            2 => "Apply",
            3 => "Force",
            4 => "Think",
            5 => "Storage",
            _ => panic!(),
        }
    }

    pub fn show_coupon(&mut self, handle: &FixHandle) {
        let ctype = self.get_coupon_type(handle);
        let lhs = self.get_coupon_lhs(handle);
        let rhs = self.get_coupon_rhs(handle);

        log::info!("type is: {ctype:?}");
        log::info!("lhs is: {lhs:?}");
        log::info!("rhs is: {rhs:?}");
    }

    fn get_coupon_type(&mut self, coupon: &FixHandle) -> &'static str {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        let handle = coupon_content.get(1);
        self.coupon_type(&handle)
    }

    pub fn get_coupon_lhs(&mut self, coupon: &FixHandle) -> FixHandle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        coupon_content.get(2)
    }

    pub fn get_coupon_rhs(&mut self, coupon: &FixHandle) -> FixHandle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        coupon_content.get(3)
    }
}
