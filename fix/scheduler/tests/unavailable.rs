#![no_std]
use fixwasm::{Blob, Error, Handle, Ref, Value};

#[fixwasm::main]
pub fn apply(value: Handle<Ref<Blob>>) -> Result<impl Value, Error> {
    fixwasm::advise(&value, fixwasm::advice::CANONICALIZE, b"")?;
    Ok(Blob::create(b"ok"))
}
