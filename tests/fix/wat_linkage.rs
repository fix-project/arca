#![no_std]

use fix::{Blob, Error, Handle, Object, Tree, Value};

#[unsafe(no_mangle)]
pub extern "C" fn increment(value: u64) -> u64 {
    value + 1
}

unsafe extern "C" {
    fn next_three(value: u64) -> u64;
}

#[fix::apply]
fn apply(
    _input: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    Ok(Blob::create(unsafe { next_three(39) }.to_le_bytes()))
}
