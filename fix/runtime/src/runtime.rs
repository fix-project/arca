use core::clone::Clone;
use core::result::Result;

use fixhandle::rawhandle::FixHandle;

pub trait DeterministicEquivRuntime {
    type BlobData: Clone + core::fmt::Debug;
    type TreeData: Clone + core::fmt::Debug;
    type Handle: Clone + core::fmt::Debug;
    type Error: core::fmt::Debug;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle;
    fn create_blob_i64(&mut self, data: u64) -> Self::Handle;
    fn create_blob(&mut self, data: Self::BlobData) -> Self::Handle;
    fn create_tree(&mut self, data: Self::TreeData) -> Self::Handle;

    fn get_blob(&self, handle: &Self::Handle) -> Result<Self::BlobData, Self::Error>;
    fn get_tree(&self, handle: &Self::Handle) -> Result<Self::TreeData, Self::Error>;

    fn is_blob(handle: &Self::Handle) -> bool;
    fn is_tree(handle: &Self::Handle) -> bool;

    fn is_equal(lhs: &Self::Handle, rhs: &Self::Handle) -> bool;
}

pub trait ExecutionRuntime: DeterministicEquivRuntime {
    fn request_execution(&mut self, combination: &Self::Handle) -> Result<(), Self::Error>;
}

pub trait Executor {
    fn execute(&mut self, combination: &FixHandle) -> FixHandle;
}

#[repr(u32)]
pub enum CouponTrades {
    EqTree = 0,
    EqApplication = 1,
    ForceResultEq = 2,
    EqStrictEncode = 3,
    ThinkApplication = 4,
    ThinkToForce = 5,
    ForceToEncodeStric = 6,
    EvalEq = 7,
    EvalBlobObj = 8,
    EvalTreeObj = 9,
    EqSym = 10,
    EqTrans = 11,
    EqSelf = 12,
}

pub trait CouponHelper: DeterministicEquivRuntime {
    fn read_blob(blob: &Self::BlobData, offset: usize, buf: &mut [u8]) -> usize;
    fn get_tree_entry(tree: &Self::TreeData, offset: usize) -> Self::Handle;

    fn trade(
        &mut self,
        trade_type: CouponTrades,
        coupons: FixHandle,
        lhs: FixHandle,
        rhs: FixHandle,
    ) -> FixHandle;

    fn coupon_type(&mut self, handle: &Self::Handle) -> &'static str {
        let mut arr = [0u8; 4];
        let blob = self.get_blob(handle).expect("Coupon type not a blob");
        Self::read_blob(&blob, 0, &mut arr);
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
        let handle = Self::get_tree_entry(&coupon_content, 1);
        self.coupon_type(&handle)
    }

    fn get_coupon_lhs(&mut self, coupon: &Self::Handle) -> Self::Handle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        Self::get_tree_entry(&coupon_content, 2)
    }

    fn get_coupon_rhs(&mut self, coupon: &Self::Handle) -> Self::Handle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        Self::get_tree_entry(&coupon_content, 3)
    }
}
