#![no_std]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use fix::{Error, Handle, Object, Tree, Value, tree};

#[fix::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Object<Tree>> + 'a, Error> {
    if combination.len()? != 4 {
        return Err(Error::WrongType);
    }
    let length = Handle::<Object<Tree>, _>::try_from(combination.get(3))?.len()?;
    Ok(Tree::from_iter((0..length).map(move |index| {
        let main_blob = combination.get(1);
        let arg_1 = combination.get(2);
        let array = Handle::<Object<Tree>, _>::try_from(combination.get(3))?;
        Ok(tree![main_blob, arg_1, array.get(index)]
            .reference()
            .application())
    })))
}
