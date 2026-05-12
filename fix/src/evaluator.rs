use fixhandle::rawhandle::{Encode, FixHandle, Object, Ref, Thunk, TreeName};

use fixruntime::{
    fixruntime::FixRuntime,
    runtime::{CouponHelper, DeterministicEquivRuntime, Executor},
};

use common::bitpack::BitPack;
use kernel::prelude::*;

fn apply(runtime: &mut FixRuntime, combination: &TreeName) -> FixHandle {
    let handle = FixHandle::Object(Object::TreeObj(*combination));
    runtime.execute(&handle)
}

fn lift(_runtime: &mut FixRuntime, handle: &FixHandle) -> FixHandle {
    match handle {
        FixHandle::Ref(r) => match r {
            Ref::TreeRef(t) => FixHandle::Object(Object::TreeObj(*t)),
            Ref::BlobRef(b) => FixHandle::Object(Object::BlobObj(*b)),
        },
        _ => *handle,
    }
}

fn lower(_runtime: &mut FixRuntime, handle: &FixHandle) -> FixHandle {
    match handle {
        FixHandle::Object(r) => match r {
            Object::TreeObj(t) => FixHandle::Ref(Ref::TreeRef(*t)),
            Object::BlobObj(b) => FixHandle::Ref(Ref::BlobRef(*b)),
        },
        _ => *handle,
    }
}

fn think(runtime: &mut FixRuntime, thunk: &Thunk) -> FixHandle {
    match thunk {
        Thunk::Identification(_) => todo!(),
        Thunk::Selection(_) => todo!(),
        Thunk::Application(tree) => {
            let eval_coupon = eval(runtime, FixHandle::Object(Object::TreeObj(*tree)));
            let rhs = runtime.get_coupon_rhs(&eval_coupon);
            let rhs = rhs.unwrap_object().unwrap_tree_obj();
            let apply_coupon = apply(runtime, &rhs);

            let mut coupons = Vec::with_capacity(32 * 2);
            coupons.extend_from_slice(&eval_coupon.pack());
            coupons.extend_from_slice(&apply_coupon.pack());
            let coupons = runtime.create_tree(Blob::new(coupons));
            let result = runtime.get_coupon_rhs(&apply_coupon);
            runtime.trade(
                fixruntime::runtime::CouponTrades::ThinkApplication,
                coupons,
                FixHandle::Thunk(*thunk),
                result,
            )
        }
    }
}

fn force(runtime: &mut FixRuntime, thunk: &Thunk) -> FixHandle {
    let think_coupon = think(runtime, thunk);
    let result = runtime.get_coupon_rhs(&think_coupon);
    match result {
        FixHandle::Object(_) | FixHandle::Ref(_) => {
            let mut coupons = Vec::with_capacity(32);
            coupons.extend_from_slice(&think_coupon.pack());
            let coupons = runtime.create_tree(Blob::new(coupons));

            runtime.trade(
                fixruntime::runtime::CouponTrades::ThinkToForce,
                coupons,
                FixHandle::Thunk(*thunk),
                result,
            )
        }
        FixHandle::Thunk(_) | FixHandle::Encode(_) => todo!(),
    }
}

fn encode(runtime: &mut FixRuntime, encode: &Encode) -> FixHandle {
    match encode {
        Encode::Strict(thunk) => {
            let force_coupon = force(runtime, thunk);
            let result = runtime.get_coupon_rhs(&force_coupon);
            let result = lift(runtime, &result);

            let mut coupons = Vec::with_capacity(32);
            coupons.extend_from_slice(&force_coupon.pack());
            let coupons = runtime.create_tree(Blob::new(coupons));

            runtime.trade(
                fixruntime::runtime::CouponTrades::ForceToEncodeStric,
                coupons,
                FixHandle::Encode(*encode),
                result,
            )
        }
        Encode::Shallow(_) => todo!(),
    }
}

fn eval_tree(runtime: &mut FixRuntime, handle: &TreeName) -> FixHandle {
    let tree_handle = FixHandle::from(Object::from(*handle));
    let tree = runtime.get_tree(&tree_handle).unwrap();
    let tree_len = FixRuntime::<'_>::get_tree_len(&tree);

    let mut coupons = Vec::with_capacity(tree_len * 32);
    let mut new_tree = Vec::with_capacity(tree_len * 32);

    for i in 0..tree_len {
        let eval_coupon = eval(runtime, FixRuntime::<'_>::get_tree_entry(&tree, i));
        coupons.extend_from_slice(&eval_coupon.pack());
        new_tree.extend_from_slice(&runtime.get_coupon_rhs(&eval_coupon).pack());
    }

    let coupons = runtime.create_tree(Blob::new(coupons));
    let new_tree = runtime.create_tree(Blob::new(new_tree));

    runtime.trade(
        fixruntime::runtime::CouponTrades::EvalTreeObj,
        coupons,
        tree_handle,
        new_tree,
    )
}

pub fn eval(runtime: &mut FixRuntime, handle: FixHandle) -> FixHandle {
    match handle {
        FixHandle::Thunk(_) | FixHandle::Ref(_) => todo!(),
        FixHandle::Object(obj) => match obj {
            Object::BlobObj(_) => {
                let coupons = vec![0u8; 0];
                let coupons = runtime.create_tree(Blob::new(coupons));
                runtime.trade(
                    fixruntime::runtime::CouponTrades::EvalBlobObj,
                    coupons,
                    handle,
                    handle,
                )
            }
            Object::TreeObj(tree) => eval_tree(runtime, &tree),
        },
        FixHandle::Encode(e) => {
            let eq_coupon = encode(runtime, &e);
            let result = runtime.get_coupon_rhs(&eq_coupon);
            let eval_coupon = eval(runtime, result);

            let eq_lhs = runtime.get_coupon_lhs(&eq_coupon);
            let eq_rhs = runtime.get_coupon_rhs(&eq_coupon);

            let mut coupons = Vec::with_capacity(32);
            coupons.extend_from_slice(&eq_coupon.pack());
            let coupons = runtime.create_tree(Blob::new(coupons));
            let eq_coupon = runtime.trade(
                fixruntime::runtime::CouponTrades::EqSym,
                coupons,
                eq_rhs,
                eq_lhs,
            );
            let result = runtime.get_coupon_rhs(&eval_coupon);

            let mut coupons = Vec::with_capacity(2 * 32);
            coupons.extend_from_slice(&eval_coupon.pack());
            coupons.extend_from_slice(&eq_coupon.pack());
            let coupons = runtime.create_tree(Blob::new(coupons));
            return runtime.trade(
                fixruntime::runtime::CouponTrades::EvalEq,
                coupons,
                handle,
                result,
            );
        }
    }
}
