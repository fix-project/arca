#![no_std]
use fix::{Blob, Error, Handle, Object, Tree, Value};

#[fix::apply]
fn apply(
    combination: &Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Blob>>, Error> {
    if combination.len()? != 3 {
        return Err(Error::WrongType);
    }
    assert!(combination.get(1).equals(combination.get(2))?);
    Ok(Blob::create(42u64.to_le_bytes()))
}
