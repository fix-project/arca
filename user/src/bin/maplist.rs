#![no_std]
#![no_main]

extern crate user;

use user::prelude::*;

/// Map a Lambda over a list, represented as cons cells.
#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let function = os::prompt();
    let DynValue::Lambda(function) = function.into() else {
        panic!("incorrect argument type to map");
    };
    let list = os::prompt();

    os::exit(map(function, list));
}

fn map(function: Ref<Lambda>, over: Ref<Value>) -> Ref<Value> {
    let mut tree = match DynValue::from(over) {
        DynValue::Null(null) => return null.into(),
        DynValue::Tree(tree) => tree,
        _ => panic!("incorrect argument type to map"),
    };
    assert_eq!(tree.len(), 2);
    let car = tree.take(0);
    let cdr = tree.take(1);
    let mapped_car = function.clone().apply(car);

    let mapf = Ref::<Lambda>::from(|x| map(function, x));
    let mapped_cdr = mapf.apply(cdr);
    let tree = os::tree(&mut [mapped_car.into(), mapped_cdr.into()]);
    tree.into()
}
