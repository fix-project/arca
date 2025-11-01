#![no_main]
#![no_std]
#![feature(try_blocks)]
#![feature(try_trait_v2)]
#![feature(iterator_try_collect)]
#![feature(box_patterns)]
#![feature(never_type)]
#![allow(dead_code)]

use kernel::prelude::*;

extern crate alloc;

#[kmain]
async fn main(_: &[usize]) {
    let arca = Arca::new();
    let func = Function::from(arca);
    let val = Value::Function(func);
    let bytes_vec = postcard::to_allocvec(&val).unwrap();
    log::info!("{}", bytes_vec.len());
    log::info!("{:?}", bytes_vec);
    let new: Value = postcard::from_bytes(&bytes_vec).unwrap();
    log::info!("{:?}", new);
    assert_eq!(new, val);
}
