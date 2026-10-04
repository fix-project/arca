#![no_std]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
use fix::{Any, Error, Handle, Object, Tree, Value, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + 'a, Error> {
    if combination.len()? != 4 {
        return Err(Error::WrongType);
    }
    let target: Handle<Object<Tree>, _> = combination.get(3).try_into()?;
    let length = target.len()?;
    let split = length / 2;
    let reduce = |start, end| {
        tree![
            combination.get(0),
            combination.get(1),
            combination.get(2),
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
            tree![combination.get(1), target.get(0)]
                .reference()
                .application(),
            tree![
                combination.get(2),
                reduce(0u64, split as u64),
                reduce(split as u64, length as u64)
            ]
            .reference()
            .application(),
        )
        .erase(),
    ))
}
