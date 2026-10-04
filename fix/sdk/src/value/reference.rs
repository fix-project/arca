use super::*;
#[derive(Clone, Copy)]
pub struct Reference<V>(pub(super) V);
impl<D: Data, V: Value<Type = Object<D>>> Value for Reference<V> {
    type Type = Ref<D>;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.value_type_at(path)?;
        self.0.build_at(None)?;
        status(unsafe { fix_sdk_reference() })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ())?;
        if !(self.0.value_type()? == ValueType::Object) {
            return Err(Error::WrongType);
        }
        Ok(ValueType::Ref)
    }
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        self.value_type_at(path)?;
        self.0.data_type()
    }
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<usize, Error> {
        self.value_type_at(path)?;
        self.0.len()
    }
}
