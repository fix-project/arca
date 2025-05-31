#![no_main]
#![no_std]
#![feature(allocator_api)]

use alloc::vec;
use kernel::prelude::*;
use kernel::rt;
use macros::kmain;

extern crate alloc;
extern crate kernel;

const IDENTITY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_identity"));
const ADD: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const CURRY: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_curry"));

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

    let x = Value::Atom(Atom::new("foo").into());
    let y = id.apply(x.clone()).run();
    assert_eq!(x, y);

    let x = Value::Tree(vec![Value::Word(10), Value::Word(20)].into());
    let y = add.clone().apply(x).run();
    assert_eq!(y, Value::Word(30));

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
    let y = cadd.apply(Value::Word(20)).run();
    assert_eq!(y, Value::Word(30));
}
