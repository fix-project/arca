use fixhandle::rawhandle::{Encode, FixHandle, Object, Ref, Thunk, TreeName};

use fixruntime::{
    fixruntime::FixRuntime,
    runtime::{CouponCollector, DeterministicEquivRuntime, Executor},
};

use common::bitpack::BitPack;
use kernel::prelude::*;

fn coupon_type(runtime: &mut FixRuntime, handle: &FixHandle) -> &'static str {
    let mut arr = [0u8; 4];
    let blob = runtime.get_blob(handle).expect("Coupon type not a blob");
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

pub fn show_coupon(runtime: &mut FixRuntime, handle: &FixHandle) {
    let lhs = get_coupon_lhs(runtime, handle);
    let rhs = get_coupon_rhs(runtime, handle);

    let coupon_content = runtime.get_tree(handle).expect("Coupon not a tree");

    let entry: Blob = coupon_content
        .get(1)
        .try_into()
        .expect("author not a handle");
    let mut handle_scratch: [u8; 32] = [0; 32];
    entry.read(0, &mut handle_scratch);
    let handle = FixHandle::unpack(handle_scratch);
    let ctype = coupon_type(runtime, &handle);

    log::info!("type is: {ctype:?}");
    log::info!("lhs is: {lhs:?}");
    log::info!("rhs is: {rhs:?}");
}

fn get_tree_entry(tree: &Tuple, idx: usize) -> FixHandle {
    let mut scratch: [u8; 32] = [0; 32];
    let entry: Blob = tree.get(idx).try_into().expect("tree entry not a blob");
    entry.read(0, &mut scratch);
    FixHandle::unpack(scratch)
}

pub fn get_coupon_lhs(runtime: &mut FixRuntime, coupon: &FixHandle) -> FixHandle {
    let coupon_content = runtime.get_tree(coupon).expect("Coupon not a tree");
    get_tree_entry(&coupon_content, 2)
}

pub fn get_coupon_rhs(runtime: &mut FixRuntime, coupon: &FixHandle) -> FixHandle {
    let coupon_content = runtime.get_tree(coupon).expect("Coupon not a tree");
    get_tree_entry(&coupon_content, 3)
}

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
            let rhs = get_coupon_rhs(runtime, &eval_coupon);
            let rhs = rhs.unwrap_object().unwrap_tree_obj();
            let apply_coupon = apply(runtime, &rhs);

            let mut coupons = Tuple::new(2);
            coupons.set(0, Blob::new(eval_coupon.pack()));
            coupons.set(1, Blob::new(apply_coupon.pack()));
            let coupons = runtime.create_tree(coupons);
            let result = get_coupon_rhs(runtime, &apply_coupon);
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
    let result = get_coupon_rhs(runtime, &think_coupon);
    match result {
        FixHandle::Object(_) | FixHandle::Ref(_) => {
            let mut coupons = Tuple::new(1);
            coupons.set(0, Blob::new(think_coupon.pack()));
            let coupons = runtime.create_tree(coupons);

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
            let result = get_coupon_rhs(runtime, &force_coupon);
            let result = lift(runtime, &result);

            let mut coupons = Tuple::new(1);
            coupons.set(0, Blob::new(force_coupon.pack()));
            let coupons = runtime.create_tree(coupons);

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

    let mut coupons = Tuple::new(tree.len());
    let mut new_tree = Tuple::new(tree.len());

    for i in 0..tree.len() {
        let eval_coupon = eval(runtime, get_tree_entry(&tree, i));
        coupons.set(i, Blob::new(eval_coupon.pack()));
        new_tree.set(i, Blob::new(get_coupon_rhs(runtime, &eval_coupon).pack()));
    }

    let coupons = runtime.create_tree(coupons);
    let new_tree = runtime.create_tree(new_tree);

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
                let coupons = Tuple::new(0);
                let coupons = runtime.create_tree(coupons);
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
            let result = get_coupon_rhs(runtime, &eq_coupon);
            let eval_coupon = eval(runtime, result);

            let eq_lhs = get_coupon_lhs(runtime, &eq_coupon);
            let eq_rhs = get_coupon_rhs(runtime, &eq_coupon);
            let mut coupons = Tuple::new(1);
            coupons.set(0, Blob::new(eq_coupon.pack()));
            let coupons = runtime.create_tree(coupons);
            let eq_coupon = runtime.trade(
                fixruntime::runtime::CouponTrades::EqSym,
                coupons,
                eq_rhs,
                eq_lhs,
            );
            let result = get_coupon_rhs(runtime, &eval_coupon);

            let mut coupons = Tuple::new(2);
            coupons.set(0, Blob::new(eval_coupon.pack()));
            coupons.set(1, Blob::new(eq_coupon.pack()));
            let coupons = runtime.create_tree(coupons);
            return runtime.trade(
                fixruntime::runtime::CouponTrades::EvalEq,
                coupons,
                handle,
                result,
            );
        }
    }
}
