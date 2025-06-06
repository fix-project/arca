#![no_main]
#![no_std]
#![feature(allocator_api)]

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
    log::info!("setting up");
    let id: Lambda = Thunk::from_elf(IDENTITY).run().try_into().unwrap();
    let add: Lambda = Thunk::from_elf(ADD).run().try_into().unwrap();
    let curry: Lambda = Thunk::from_elf(CURRY).run().try_into().unwrap();
    let map: Lambda = Thunk::from_elf(MAP).run().try_into().unwrap();

    // identity function
    log::info!("testing id");
    let x = Value::Atom(Atom::new("foo"));
    let y = id.apply(x.clone()).run();
    assert_eq!(x, y);

    // add
    log::info!("testing add");
    let x = Tree::new([10.into(), 20.into()]);
    let y = add.clone().apply(x.into()).run();
    assert_eq!(y, 30.into());

    // curry add
    log::info!("testing curry");
    let x = add.clone();
    let n = 2;
    let Value::Lambda(cadd) = curry.apply(x.into()).run() else {
        panic!();
    };
    let Value::Lambda(cadd) = cadd.apply(n.into()).run() else {
        panic!();
    };
    let Value::Lambda(cadd) = cadd.apply(10.into()).run() else {
        panic!();
    };
    let y = cadd.clone().apply(20.into()).run();
    assert_eq!(y, 30.into());

    // map
    log::info!("testing map");
    let tuple = Tree::new([1.into(), 2.into(), 3.into(), 4.into()]).into();
    let Value::Lambda(map) = map.apply(Value::Lambda(cadd)).run() else {
        panic!();
    };
    let result = map.apply(tuple).run();
    let result = eval(result);
    let expected = Tree::new([11.into(), 12.into(), 13.into(), 14.into()]).into();
    assert_eq!(result, expected);
}

pub fn eval(x: Value) -> Value {
    match x {
        Value::Error(value) => Error::new(eval(value.into())).into(),
        Value::Tree(values) => Value::Tree(values.iter().map(|x| eval(x.clone())).collect()),
        Value::Thunk(thunk) => eval(thunk.run()),
        x => x,
    }
}
