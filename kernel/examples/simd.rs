#![no_std]
#![no_main]

use common::elfloader;
use kernel::prelude::*;

const USER: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_simd"));

#[kmain]
fn main(_: &[usize]) {
    let user: Function = elfloader::load_elf(USER).unwrap();
    let x = Blob::new(&[28u8; 32]);
    let y = Blob::new(&[3u8; 32]);
    let result: Blob = user
        .apply(x)
        .apply(y)
        .force()
        .try_into()
        .expect("result was not a blob");
    assert_eq!(result, Blob::new(&[31u8; 32]));
}
