#![no_std]

use fix::{Error, Handle, Object, Thunk, Tree, Value, choose, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Thunk> + 'a, Error> {
    let closure = Handle::<Object<Tree>, _>::try_from(combination.get(0))?;
    if closure.len()? != 3 {
        return Err(Error::WrongType);
    }
    let inner = Tree::from_iter(
        (0..combination.len()?)
            .map(move |index| choose(index == 0, closure.get(2), combination.get(index))),
    );
    Ok(tree![closure.get(1), inner.application().strict()].application())
}
