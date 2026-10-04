#![no_std]
use fix::{Blob, Error, Handle, Object, Tree, Value};

#[fix::apply]
fn apply(
    combination: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    let left = Handle::<Object<Blob>, _>::try_from(combination.get(1))?.read_u64()?;
    let right = Handle::<Object<Blob>, _>::try_from(combination.get(2))?.read_u64()?;
    Ok(Blob::create(left.wrapping_add(right).to_le_bytes()))
}
