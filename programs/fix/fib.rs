#![no_std]

use fix::{Any, Blob, Error, Handle, Object, Tree, Value, choose, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    if combination.len()? != 4 {
        return Err(Error::WrongType);
    }
    let fib = combination.get(1);
    let n = Handle::<Object<Blob>, _>::try_from(combination.get(2))?.read_u64()?;
    let add = combination.get(3);
    let first = tree![Blob::create(b""), fib, n.wrapping_sub(1), add]
        .reference()
        .application()
        .strict();
    let second = tree![Blob::create(b""), fib, n.wrapping_sub(2), add]
        .reference()
        .application()
        .strict();
    let sum = tree![Blob::create(b""), add, first, second]
        .reference()
        .application()
        .erase();
    Ok(choose(
        n <= 1,
        Blob::create(1u64.to_le_bytes()).erase(),
        sum,
    ))
}
