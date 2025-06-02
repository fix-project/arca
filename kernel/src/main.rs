#![no_main]
#![no_std]
#![feature(allocator_api)]

use alloc::sync::Arc;
use alloc::vec;
use kernel::prelude::*;
use kernel::rt;
use macros::kmain;

extern crate alloc;
extern crate kernel;

const IDENTITY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const ADD: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const CURRY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_curry"));
const MAP: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_map"));

#[kmain]
async fn kmain(_: &[usize]) {
    let id = Thunk::from_elf(IDENTITY);
    let Value::Lambda(id) = id.run() else {
        panic!();
    };
    let add = Thunk::from_elf(ADD);
    let Value::Lambda(add) = add.run() else {
        panic!();
    };
    let curry = Thunk::from_elf(CURRY);
    let Value::Lambda(curry) = curry.run() else {
        panic!();
    };
    let map = Thunk::from_elf(MAP);
    let Value::Lambda(map) = map.run() else {
        panic!();
    };

    // identity function
    let x = Value::Atom(Atom::new("foo").into());
    let y = id.apply(x.clone()).run();
    assert_eq!(x, y);

    // add
    let x = Value::Tree(vec![Value::Word(10), Value::Word(20)].into());
    let y = add.clone().apply(x).run();
    assert_eq!(y, Value::Word(30));

    // curry add
    log::warn!("curry");
    let x = add.clone();
    let n = Value::Word(2);
    let Value::Lambda(cadd) = curry.apply(Value::Lambda(x)).run() else {
        panic!();
    };
    let Value::Lambda(cadd) = cadd.apply(n).run() else {
        panic!();
    };
    let Value::Lambda(cadd) = cadd.apply(Value::Word(10)).run() else {
        panic!();
    };
    let y = cadd.clone().apply(Value::Word(20)).run();
    assert_eq!(y, Value::Word(30));

    // map
    log::warn!("start");
    let tuple = Value::Tree(
        vec![
            Value::Word(1),
            Value::Word(2),
            Value::Word(3),
            Value::Word(4),
        ]
        .into(),
    );
    let Value::Lambda(map) = map.apply(Value::Lambda(cadd)).run() else {
        panic!();
    };
    let result = map.apply(tuple).run();
    log::info!("evaluating result");
    let result = eval(result);
    log::info!("{result:?}");
}

pub fn eval(x: Value) -> Value {
    match x {
        Value::Error(value) => Value::Error(eval(Arc::unwrap_or_clone(value)).into()),
        Value::Tree(values) => Value::Tree(values.into_iter().map(|x| eval(x.clone())).collect()),
        Value::Thunk(thunk) => eval(thunk.run()),
        x => x,
    }
}
