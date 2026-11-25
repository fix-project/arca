#![no_std]
use fixwasm::{Any, Error, Handle, Object, Tree, Value, tree};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<Handle<Any>, Error> {
    if combination.len()? == 3 {
        return Ok(combination.get(2));
    }
    Ok(tree![
        combination.get(2).application().strict(),
        combination.get(3).application().strict(),
        combination.get(4).application().strict(),
        combination.get(5).application().strict()
    ]
    .erase())
}
