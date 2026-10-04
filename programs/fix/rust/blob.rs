#![no_std]
use fix::{Blob, Error, Handle, Object, Tree, Value};

#[fix::apply]
fn apply(
    _combination: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    Ok(Blob::create(b"hello, world"))
}
