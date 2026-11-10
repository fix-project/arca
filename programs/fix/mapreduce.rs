#![no_std]
use fixwasm::{Any, Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    if combination.len()? != 5 {
        return Err(Error::WrongType);
    }
    let target: Handle<Object<Tree>> = combination.get(4).try_into()?;
    let length = target.len()?;
    let split = length / 2;
    let reduce = |start, end| {
        tree![
            fixwasm::Blob::create(b""),
            combination.get(1),
            combination.get(2),
            combination.get(3),
            tree![target, start, end].reference().selection().strict()
        ]
        .reference()
        .application()
        .strict()
    };
    Ok(if length == 0 {
        tree![].erase()
    } else if length == 1 {
        tree![
            fixwasm::Blob::create(b""),
            combination.get(2),
            target.get(0)
        ]
        .reference()
        .application()
        .erase()
    } else {
        tree![
            fixwasm::Blob::create(b""),
            combination.get(3),
            reduce(0u64, split),
            reduce(split, length)
        ]
        .reference()
        .application()
        .erase()
    })
}
