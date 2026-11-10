#![no_std]
use fixwasm::{Blob, Error, Handle, Object, Tree, Value};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value<Type = Object<Blob>>, Error> {
    let left = Handle::<Object<Blob>>::try_from(combination.get(2))?.read_u64()?;
    let right = Handle::<Object<Blob>>::try_from(combination.get(3))?.read_u64()?;
    Ok(Blob::create(left.wrapping_add(right).to_le_bytes()))
}
