#![no_std]
use fixwasm::{Blob, Error, Handle, Object, Tree, Value};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value<Type = Object<Blob>>, Error> {
    if combination.len()? != 4 {
        return Err(Error::WrongType);
    }
    assert!(combination.get(2).equals(combination.get(3))?);
    Ok(Blob::create(42u64.to_le_bytes()))
}
