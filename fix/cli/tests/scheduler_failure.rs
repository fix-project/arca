#![no_std]

use fixwasm::{Any, Blob, Error, Handle, Object, Tree, Value};

#[fixwasm::apply]
pub fn apply(combination: &Handle<Object<Tree>>) -> Result<Handle<Any>, Error> {
    if combination.get(4).read_u64()? == 0 {
        panic!("named input cannot be loaded");
    }
    Blob::create(b"unexpected scheduler result")
        .erase()
        .capture()
}
