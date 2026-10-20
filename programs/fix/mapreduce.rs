#![no_std]
use fix::{Any, Error, Handle, Object, Tree, Value, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    if combination.len()? != 5 {
        return Err(Error::WrongType);
    }
    let target: Handle<Object<Tree>, _> = combination.get(4).try_into()?;
    let length = target.len()?;
    let split = length / 2;
    let reduce = |start, end| {
        tree![
            fix::Blob::create(b""),
            combination.get(1),
            combination.get(2),
            combination.get(3),
            tree![target, start, end].reference().selection().strict()
        ]
        .reference()
        .application()
        .strict()
    };
    Ok(fix::choose(
        length == 0,
        tree![].erase(),
        fix::choose(
            length == 1,
            tree![fix::Blob::create(b""), combination.get(2), target.get(0)]
                .reference()
                .application(),
            tree![
                fix::Blob::create(b""),
                combination.get(3),
                reduce(0u64, split),
                reduce(split, length)
            ]
            .reference()
            .application(),
        )
        .erase(),
    ))
}
