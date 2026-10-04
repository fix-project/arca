#![no_std]
use fix::{Any, Blob, Error, Handle, Object, Tree, Value, tree};

#[fix::apply]
fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + use<'a>, Error> {
    let node: Handle<Object<Tree>, _> = combination.get(1).try_into()?;
    let path: Handle<Object<Tree>, _> = combination.get(2).try_into()?;
    if !matches!(node.len()?, 0 | 3) {
        return Err(Error::WrongType);
    }
    let done = path.is_empty()?;
    let result = fix::choose(done, node, tree![]).erase();
    if done || node.is_empty()? {
        return Ok(fix::Choice::Right(result));
    }
    let (_, _, children): (Handle<Any, _>, Handle<Any, _>, Handle<Object<Tree>, _>) =
        node.try_into()?;
    let wanted: Handle<Object<Blob>, _> = path.get(0).try_into()?;
    for index in 0..children.len()? {
        let child: Handle<Object<Tree>, _> = children.get(index).try_into()?;
        let (name, _, _): (Handle<Object<Blob>, _>, Handle<Any, _>, Handle<Any, _>) =
            child.try_into()?;
        if name == wanted {
            let remaining = Tree::from_iter((1..path.len()?).map(move |i| path.get(i)));
            return Ok(fix::Choice::Left(
                tree![combination.get(0), child, remaining]
                    .reference()
                    .application()
                    .erase(),
            ));
        }
    }
    Ok(fix::Choice::Right(result))
}
