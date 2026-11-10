#![no_std]
extern crate alloc;
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use fixwasm::{Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
pub fn apply<'a>(
    combination: &'a Handle<Object<Tree>>,
) -> Result<impl Value<Type = Object<Tree>> + 'a, Error> {
    if combination.len()? != 5 {
        return Err(Error::WrongType);
    }
    let length = usize::try_from(Handle::<Object<Tree>>::try_from(combination.get(4))?.len()?)?;
    Ok(Tree::from_iter((0..length).map(move |index| {
        let main_blob = combination.get(2);
        let arg_1 = combination.get(3);
        let array = Handle::<Object<Tree>>::try_from(combination.get(4))?;
        Ok(tree![
            fixwasm::Blob::create(b""),
            main_blob,
            arg_1,
            array.get(index)
        ]
        .reference()
        .application())
    })))
}
