#![no_std]
use fixwasm::{Blob, Error, Handle, Object, Tree, Value};

#[fixwasm::apply]
fn apply(_combination: &Handle<Object<Tree>>) -> Result<impl Value<Type = Object<Blob>>, Error> {
    Ok(Blob::create(b"hello, world"))
}
