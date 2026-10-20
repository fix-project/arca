#![no_std]

use fix::{Error, Handle, Object, Thunk, Tree, Value, choose, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Thunk> + 'a, Error> {
    let closure = Handle::<Object<Tree>, _>::try_from(combination.get(1))?;
    if closure.len()? != 4 {
        return Err(Error::WrongType);
    }
    let inner = Tree::from_iter(
        (0..usize::try_from(combination.len()?)?)
            .map(move |index| choose(index == 1, closure.get(3), combination.get(index))),
    );
    Ok(tree![
        fix::Blob::create(b""),
        closure.get(2),
        inner.application().strict()
    ]
    .application())
}
