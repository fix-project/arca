#![no_std]
use fix::{Blob, Capture, Error, Handle, Ref, Value};

#[fix::main]
pub fn apply(value: Handle<Ref<Blob>, Capture>) -> Result<impl Value, Error> {
    fix::advise(&value, fix::advice::CANONICALIZE, b"")?;
    Ok(Blob::create(b"ok"))
}
