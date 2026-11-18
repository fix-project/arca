use alloc::vec::Vec;
pub use fixwasm::Value as AsValue;
pub use fixwasm::backend::Value;
pub use fixwasm::*;

pub fn capture(value: impl AsValue) -> Result<Value, Error> {
    Ok(Value::from_inner(value.capture()?))
}
pub fn as_tree(value: &Value) -> Result<Handle<Object<Tree>>, Error> {
    Handle::<Object<Tree>>::try_from(value.inner())
}
pub fn child(value: &Value, index: usize) -> Result<Value, Error> {
    capture(as_tree(value)?.get(index))
}
pub fn fields(value: &Value) -> Result<Vec<Value>, Error> {
    let tree = as_tree(value)?;
    (0..usize::try_from(tree.len()?)?)
        .map(|index| capture(tree.get(index)))
        .collect()
}
pub fn reference(value: &Value) -> Result<Value, Error> {
    if value.value_type() == ValueType::Ref {
        return Ok(value.clone());
    }
    capture(Handle::<Object<Any>>::try_from(value.inner())?.reference())
}
