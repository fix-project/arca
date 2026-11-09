#![no_std]
use fix::{Blob, Capture, Error, Handle, Object, Tree};

#[fix::main(combination)]
pub fn apply(
    combination: &Handle<Object<Tree>, fix::Focus>,
    value: Handle<Object<Blob>, Capture>,
) -> Result<Handle<Object<Blob>, Capture>, Error> {
    fix::advise(combination, "test::marker", b"seen")?;
    Ok(value)
}
