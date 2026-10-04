use super::*;
#[derive(Clone, Copy)]
pub struct Strict<V>(pub(super) V);
impl<V: Value> Value for Strict<V> {
    type Type = Encode;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.value_type_at(path)?;
        self.0.build_at(None)?;
        status(unsafe { fix_sdk_strict() })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ())?;
        if !(self.0.value_type()? == ValueType::Thunk) {
            return Err(Error::WrongType);
        }
        Ok(ValueType::Encode)
    }
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.value_type_at(path)?;
        Ok(EncodeType::Strict)
    }
}
#[derive(Clone, Copy)]
pub struct Shallow<V>(pub(super) V);
impl<V: Value> Value for Shallow<V> {
    type Type = Encode;
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
        self.value_type_at(path)?;
        self.0.build_at(None)?;
        status(unsafe { fix_sdk_shallow() })
    }
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        at_root(path, ())?;
        if !(self.0.value_type()? == ValueType::Thunk) {
            return Err(Error::WrongType);
        }
        Ok(ValueType::Encode)
    }
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        self.value_type_at(path)?;
        Ok(EncodeType::Shallow)
    }
}
