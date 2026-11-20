#![no_std]
use fixwasm::{Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<impl Value, Error> {
    Ok(tree![
        combination.get(2).application().strict(),
        combination.get(3).application().strict(),
        combination.get(4).application().strict(),
        combination.get(5).application().strict()
    ])
}
