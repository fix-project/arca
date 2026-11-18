#![no_std]
use fixwasm::{Any, Error, Handle, Object, Tree, Value as AsValue};

#[fixwasm::apply]
fn apply(combination: &Handle<Object<Tree>>) -> Result<Handle<Any>, Error> {
    let target = combination.get(2);
    for (index, key) in [(3, "fix::output-size"), (4, "fix::parallel")] {
        let payload = combination.get(index);
        let mut bytes = [0; 8];
        let length = usize::try_from(payload.len()?.min(8))?;
        payload.read_into(0, &mut bytes[..length])?;
        fixwasm::advise(&target, key, &bytes[..length])?;
    }
    Ok(target.application().strict().erase())
}
