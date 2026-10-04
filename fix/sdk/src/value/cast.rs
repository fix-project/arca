use super::*;
pub struct Cast<T, V>(V, PhantomData<T>);
impl<T, V: Clone> Clone for Cast<T, V> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}
impl<T, V: Copy> Copy for Cast<T, V> {}
impl<T: FixType, V: Value> Value for Cast<T, V> {
    type Type = T;
    delegate_value!(this, &this.0);
}
pub(crate) fn checked<T: FixType, V: Value>(value: V) -> Result<Cast<T, V>, Error> {
    if let Some(kind) = T::VALUE
        && value.value_type()? != kind
    {
        return Err(Error::WrongType);
    }
    if let Some(kind) = T::DATA
        && value.data_type()? != kind
    {
        return Err(Error::WrongType);
    }
    Ok(Cast(value, PhantomData))
}
impl<T: FixType, V: Value> TryFrom<Select<V>> for Cast<T, Select<V>> {
    type Error = Error;
    fn try_from(value: Select<V>) -> Result<Self, Error> {
        checked(value)
    }
}
impl<T: FixType, V: Value> TryFrom<Erased<V>> for Cast<T, Erased<V>> {
    type Error = Error;
    fn try_from(value: Erased<V>) -> Result<Self, Error> {
        checked(value)
    }
}
impl<'a, T: FixType> TryFrom<&'a dyn Value<Type = Any>> for Cast<T, &'a dyn Value<Type = Any>> {
    type Error = Error;
    fn try_from(value: &'a dyn Value<Type = Any>) -> Result<Self, Error> {
        checked(value)
    }
}
#[cfg(feature = "alloc")]
impl<'a, T: FixType> TryFrom<alloc::rc::Rc<dyn Value<Type = Any> + 'a>>
    for Cast<T, alloc::rc::Rc<dyn Value<Type = Any> + 'a>>
{
    type Error = Error;
    fn try_from(value: alloc::rc::Rc<dyn Value<Type = Any> + 'a>) -> Result<Self, Error> {
        checked(value)
    }
}
