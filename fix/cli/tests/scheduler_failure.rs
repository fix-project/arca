#![no_std]

use fixwasm::{Any, Blob, Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
pub fn apply(combination: &Handle<Object<Tree>>) -> Result<Handle<Any>, Error> {
    let input = if combination.get(2).equals(Blob::create(b"serve"))? {
        let reply =
            Handle::<Object<Tree>>::try_from(tree![Blob::create(b"session_next")].effect()?)?;
        let request = Handle::<Object<Tree>>::try_from(reply.get(1))?;
        let combination = Handle::<Object<Tree>>::try_from(request.get(1))?;
        combination.get(4).read_u64()?
    } else {
        combination.get(4).read_u64()?
    };
    if input == 0 {
        panic!("named input cannot be loaded");
    }
    Blob::create(b"unexpected scheduler result")
        .erase()
        .capture()
}
