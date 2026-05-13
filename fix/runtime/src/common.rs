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

impl TryFrom<usize> for CouponTrades {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(CouponTrades::EqTree),
            1 => Ok(CouponTrades::EqApplication),
            2 => Ok(CouponTrades::ForceResultEq),
            3 => Ok(CouponTrades::EqStrictEncode),
            4 => Ok(CouponTrades::ThinkApplication),
            5 => Ok(CouponTrades::ThinkToForce),
            6 => Ok(CouponTrades::ForceToEncodeStric),
            7 => Ok(CouponTrades::EvalEq),
            8 => Ok(CouponTrades::EvalBlobObj),
            9 => Ok(CouponTrades::EvalTreeObj),
            10 => Ok(CouponTrades::EqSym),
            11 => Ok(CouponTrades::EqTrans),
            12 => Ok(CouponTrades::EqSelf),
            _ => Err(()),
        }
    }
}

#[allow(unused)]
#[repr(usize)]
pub enum FixOp {
    Eval = 0,
    Trade = 1,
    Apply = 2,
}

impl TryFrom<usize> for FixOp {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(FixOp::Eval),
            1 => Ok(FixOp::Trade),
            2 => Ok(FixOp::Apply),
            _ => Err(()),
        }
    }
}
