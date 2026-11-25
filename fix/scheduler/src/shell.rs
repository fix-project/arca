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
    Ok(as_tree(value)?
        .get_all()?
        .into_iter()
        .map(Value::from_inner)
        .collect())
}
pub fn reference(value: &Value) -> Result<Value, Error> {
    if value.value_type() == ValueType::Ref {
        return Ok(value.clone());
    }
    capture(Handle::<Object<Any>>::try_from(value.inner())?.reference())
}
