use alloc::{rc::Rc, vec::Vec};
pub use fix::*;

pub type Root = Rc<Handle<Any, Capture>>;
pub fn root(value: impl Value) -> Result<Root, Error> {
    Ok(Rc::new(value.capture()?))
}
pub fn as_tree(value: &Root) -> Result<Handle<Object<Tree>, &Capture>, Error> {
    Handle::<Object<Tree>, _>::try_from(value.as_ref())
}
pub fn child(value: &Root, index: usize) -> Result<Root, Error> {
    root(as_tree(value)?.get(index))
}

pub fn fields(value: &Root) -> Result<Vec<Root>, Error> {
    let tree = as_tree(value)?;
    (0..usize::try_from(tree.len()?)?)
        .map(|index| root(tree.get(index)))
        .collect()
}

pub fn reference(value: &Root) -> Result<Root, Error> {
    if value.value_type()? == ValueType::Ref {
        return Ok(value.clone());
    }
    root(Handle::<Object<Any>, _>::try_from(value.as_ref())?.reference())
}
