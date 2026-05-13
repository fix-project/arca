use core::result::Result;
use core::{clone::Clone, panic};

use common::bitpack::BitPack;
use fixhandle::rawhandle::{Encode, FixHandle, Object, Thunk};

use crate::vmcommon::CouponTrades;

#[allow(unused)]
pub trait DeterministicEquivRuntime {
    type Handle: Clone + core::fmt::Debug;
    type Error: core::fmt::Debug;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle;
    fn create_blob_i64(&mut self, data: u64) -> Self::Handle;
    fn create_blob(&mut self, data: &[u8]) -> Self::Handle;
    fn create_tree(&mut self, data: &[u8]) -> Self::Handle;

    fn get_blob(&self, handle: &Self::Handle) -> Result<&[u8], Self::Error>;
    fn get_tree(&self, handle: &Self::Handle) -> Result<&[u8], Self::Error>;
}

#[allow(unused)]
pub trait CouponHelper: DeterministicEquivRuntime<Handle = FixHandle> {
    fn read_blob(blob: &[u8], offset: usize, buf: &mut [u8]) -> usize {
        let n = (blob.len() - offset).min(buf.len());
        buf[..n].copy_from_slice(&blob[offset..offset + n]);
        n
    }

    fn get_tree_entry(tree: &[u8], offset: usize) -> Self::Handle {
        let mut scratch: [u8; 32] = [0; 32];
        if offset < Self::get_tree_len(tree) {
            scratch.copy_from_slice(&tree[offset * 32..(offset + 1) * 32]);
        } else {
            panic!();
        }

        FixHandle::unpack(scratch)
    }

    fn get_tree_len(tree: &[u8]) -> usize {
        tree.len() / 32
    }

    fn coupon_type(&mut self, handle: &Self::Handle) -> &'static str {
        let mut arr = [0u8; 4];
        let blob = self.get_blob(handle).expect("Coupon type not a blob");
        Self::read_blob(blob, 0, &mut arr);
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

    fn show_coupon(&mut self, handle: &Self::Handle) {
        let ctype = self.get_coupon_type(handle);
        let lhs = self.get_coupon_lhs(handle);
        let rhs = self.get_coupon_rhs(handle);

        log::info!("type is: {ctype:?}");
        log::info!("lhs is: {lhs:?}");
        log::info!("rhs is: {rhs:?}");
    }

    fn get_coupon_type(&mut self, coupon: &Self::Handle) -> &'static str {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        let handle = Self::get_tree_entry(coupon_content, 1);
        self.coupon_type(&handle)
    }

    fn get_coupon_lhs(&mut self, coupon: &Self::Handle) -> Self::Handle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        Self::get_tree_entry(coupon_content, 2)
    }

    fn get_coupon_rhs(&mut self, coupon: &Self::Handle) -> Self::Handle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        Self::get_tree_entry(coupon_content, 3)
    }
}

#[allow(unused)]
pub trait Operator {
    fn trade(
        &mut self,
        trade_type: CouponTrades,
        coupons: FixHandle,
        lhs: FixHandle,
        rhs: FixHandle,
    ) -> FixHandle;

    fn eval(&mut self, handle: FixHandle) -> FixHandle;

    fn apply(&mut self, handle: FixHandle) -> FixHandle;
}

#[allow(unused)]
pub trait Visitor: CouponHelper<Handle = FixHandle> {
    fn visit<F>(&self, handle: FixHandle, callback: &mut F)
    where
        F: FnMut(&Self, &FixHandle),
    {
        match handle {
            FixHandle::Encode(e) => match e {
                Encode::Strict(t) | Encode::Shallow(t) => self.visit(FixHandle::Thunk(t), callback),
            },
            FixHandle::Ref(_) => todo!(),
            FixHandle::Thunk(t) => match t {
                Thunk::Application(tree) => {
                    self.visit(FixHandle::Object(Object::from(tree)), callback)
                }
                _ => todo!(),
            },
            FixHandle::Object(obj) => match obj {
                Object::BlobObj(_) => {}
                Object::TreeObj(_) => {
                    let tree_data = self.get_tree(&handle).unwrap();
                    for i in 0..Self::get_tree_len(tree_data) {
                        let child = Self::get_tree_entry(tree_data, i);
                        self.visit(Self::get_tree_entry(tree_data, i), callback);
                    }
                }
            },
        };
        callback(self, &handle);
    }
}
