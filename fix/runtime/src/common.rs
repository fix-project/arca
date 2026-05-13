#[allow(unused)]
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

#[allow(unused)]
#[repr(usize)]
pub enum FixOp {
    Eval = 0,
    Trade = 1,
    Apply = 2,
}
