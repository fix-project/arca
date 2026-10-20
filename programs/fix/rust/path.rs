#![no_std]
use fix::{
    Any, Blob,
    Choice::{Left, Right},
    Error, Handle, Object, Tree, Value, ValueType, tree,
};

#[fix::apply]
fn apply<'a>(
    combination: &'a Handle<Object<Tree>, fix::Focus>,
) -> Result<impl Value<Type = Any> + use<'a>, Error> {
    let node: Handle<Object<Tree>, _> = combination.get(2).try_into()?;
    let path: Handle<Object<Tree>, _> = combination.get(3).try_into()?;
    if !matches!(node.len()?, 0 | 3) {
        return Err(Error::WrongType);
    }
    let done = path.is_empty()?;
    let result = fix::choose(done, node, tree![]).erase();
    if done || node.is_empty()? {
        return Ok(Right(Right(result)));
    }
    let (name, stat, children): (Handle<Any, _>, Handle<Any, _>, Handle<Any, _>) =
        node.try_into()?;
    if children.value_type()? == ValueType::Ref {
        return Ok(Left(
            tree![
                combination.get(0),
                combination.get(1),
                tree![name, stat, children.identification().strict()],
                path
            ]
            .application()
            .erase(),
        ));
    }
    let children: Handle<Object<Tree>, _> = children.try_into()?;
    let wanted: Handle<Object<Blob>, _> = path.get(0).try_into()?;
    for index in 0..usize::try_from(children.len()?)? {
        let child: Handle<Object<Tree>, _> = children.get(index).try_into()?;
        let (name, _, _): (Handle<Object<Blob>, _>, Handle<Any, _>, Handle<Any, _>) =
            child.try_into()?;
        if name == wanted {
            let remaining =
                Tree::from_iter((1..usize::try_from(path.len()?)?).map(move |i| path.get(i)));
            return Ok(Right(Left(
                tree![Blob::create(b""), combination.get(1), child, remaining]
                    .reference()
                    .application()
                    .erase(),
            )));
        }
    }
    Ok(Right(Right(result)))
}
