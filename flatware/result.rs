#![no_std]

use fix::{Any, Blob, Error, Handle, Object, Tree, Value};

#[fix::apply]
fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    let result: Handle<Object<Tree>, _> = combination.get(2).try_into()?;
    let status: Handle<Object<Blob>, _> = result.get(1).try_into()?;
    let mut bytes = [0; 4];
    status.read_into(0, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    let index: Handle<Object<Blob>, _> = combination.get(3).try_into()?;
    let descriptors: Handle<Object<Tree>, _> = result.get(0).try_into()?;
    let index = usize::try_from(index.read_u64()?).map_err(|_| Error::OutOfBounds)?;
    Ok(descriptors.get(index))
}
