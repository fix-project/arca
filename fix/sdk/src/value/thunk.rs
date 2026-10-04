use super::*;
#[derive(Clone, Copy)]
pub struct Identification<V>(pub(super) V);
impl<V: Value> Value for Identification<V> {
    type Type = Thunk;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.value_type_at(path)?;
        self.0.build_at(None)?;
        status(unsafe { fix_sdk_identification() })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ())?;
        if !(matches!(self.0.value_type()?, ValueType::Object | ValueType::Ref)) {
            return Err(Error::WrongType);
        }
        Ok(ValueType::Thunk)
    }
}
#[derive(Clone, Copy)]
pub struct Application<V>(pub(super) V);
impl<V: Value> Value for Application<V> {
    type Type = Thunk;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.value_type_at(path)?;
        self.0.build_at(None)?;
        status(unsafe { fix_sdk_application() })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ())?;
        if !(matches!(self.0.value_type()?, ValueType::Object | ValueType::Ref)
            && self.0.data_type()? == DataType::Tree)
        {
            return Err(Error::WrongType);
        }
        Ok(ValueType::Thunk)
    }
}
#[derive(Clone, Copy)]
pub struct Selection<V>(pub(super) V);
impl<V: Value> Value for Selection<V> {
    type Type = Thunk;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.value_type_at(path)?;
        self.0.build_at(None)?;
        status(unsafe { fix_sdk_selection() })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ())?;
        if !(matches!(self.0.value_type()?, ValueType::Object | ValueType::Ref)
            && self.0.data_type()? == DataType::Tree)
        {
            return Err(Error::WrongType);
        }
        Ok(ValueType::Thunk)
    }
}
