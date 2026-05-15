use common::bitpack::BitPack;
use core::{clone::Clone, fmt, panic, result::Result};
use fixhandle::rawhandle::{Encode, FixHandle, Object, Thunk};
use std::ops::Deref;

use crate::vmcommon::CouponTrades;

#[allow(unused)]
pub trait DeterministicEquivRuntime {
    type BlobData<'a>: Deref<Target = [u8]> + Clone + fmt::Debug
    where
        Self: 'a;
    type TreeData<'a>: Deref<Target = [u8]> + Clone + fmt::Debug
    where
        Self: 'a;
    type Handle: Clone + fmt::Debug;
    type Error: fmt::Debug;

    fn create_blob_i32(&mut self, data: u32) -> Self::Handle;
    fn create_blob_i64(&mut self, data: u64) -> Self::Handle;
    fn create_blob(&mut self, data: &[u8]) -> Self::Handle;
    fn create_tree(&mut self, data: &[u8]) -> Self::Handle;

    fn get_blob<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::BlobData<'a>, Self::Error>;
    fn get_tree<'a>(&'a self, handle: &'a Self::Handle) -> Result<Self::TreeData<'a>, Self::Error>;
}

#[allow(unused)]
pub trait CouponHelper: DeterministicEquivRuntime<Handle = FixHandle>
where
    for<'a> Self::BlobData<'a>: AsRef<[u8]>,
    for<'a> Self::TreeData<'a>: AsRef<[u8]>,
{
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

    fn coupon_type(&self, handle: &Self::Handle) -> &'static str {
        let mut arr = [0u8; 4];
        let blob = self.get_blob(handle).expect("Coupon type not a blob");
        Self::read_blob(blob.as_ref(), 0, &mut arr);
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

    fn show_coupon(&mut self, handle: &Self::Handle) -> String {
        let ctype = self.get_coupon_type(handle);
        let lhs = self.get_coupon_lhs(handle);
        let rhs = self.get_coupon_rhs(handle);

        format!("{lhs:?} --{ctype}-- {rhs:?}")
    }

    fn get_coupon_type(&self, coupon: &Self::Handle) -> &'static str {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        let handle = Self::get_tree_entry(coupon_content.as_ref(), 1);
        self.coupon_type(&handle)
    }

    fn get_coupon_lhs(&self, coupon: &Self::Handle) -> Self::Handle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        Self::get_tree_entry(coupon_content.as_ref(), 2)
    }

    fn get_coupon_rhs(&self, coupon: &Self::Handle) -> Self::Handle {
        let coupon_content = self.get_tree(coupon).expect("Coupon not a tree");
        Self::get_tree_entry(coupon_content.as_ref(), 3)
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
pub trait Visitor: CouponHelper<Handle = FixHandle>
where
    for<'a> Self::BlobData<'a>: AsRef<[u8]>,
    for<'a> Self::TreeData<'a>: AsRef<[u8]>,
{
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
                    for i in 0..Self::get_tree_len(tree_data.as_ref()) {
                        let child = Self::get_tree_entry(tree_data.as_ref(), i);
                        self.visit(child, callback);
                    }
                }
            },
        };
        callback(self, &handle);
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    OOB,
    TypeMismatch,
    UnexpectedFunction,
    FileError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Identifier(String),
    Number(i64),
    String(String),
    LParen,
    RParen,
    Comma,
    Semicolon,
    Equals,
    Eof,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(i64),
    Identifier(String),
    String(String),
    Call { name: String, args: Vec<Expr> },
    Group(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum Statement {
    Assign { name: String, expr: Expr },
    Print(Expr),
    ShowCoupon(Expr),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Value<H, B, T> {
    Handle(H),
    BlobData(B),
    TreeData(T),
    Int(i64),
    String(String),
    Unit,
}

impl<H, B, T> fmt::Display for Value<H, B, T>
where
    H: fmt::Debug,
    B: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Handle(handle) => write!(formatter, "{handle:?}"),
            Self::BlobData(data) => write!(formatter, "{data:?}"),
            Self::TreeData(data) => write!(formatter, "{data:?}"),
            Self::Int(value) => write!(formatter, "{value}"),
            Self::String(value) => write!(formatter, "{value}"),
            Self::Unit => write!(formatter, "()"),
        }
    }
}
