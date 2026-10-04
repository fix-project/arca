use super::*;
pub enum Any {}
pub enum Blob {}
pub enum Tree {}
pub struct Object<T = Any>(PhantomData<T>);
pub struct Ref<T = Any>(PhantomData<T>);
pub enum Thunk {}
pub enum Encode {}

pub trait FixType {
    const VALUE: Option<ValueType>;
    const DATA: Option<DataType> = None;
}
pub trait Data {
    const KIND: Option<DataType>;
}
impl FixType for Any {
    const VALUE: Option<ValueType> = None;
}
impl Data for Any {
    const KIND: Option<DataType> = None;
}
impl Data for Blob {
    const KIND: Option<DataType> = Some(DataType::Blob);
}
impl Data for Tree {
    const KIND: Option<DataType> = Some(DataType::Tree);
}
impl<T: Data> FixType for Object<T> {
    const VALUE: Option<ValueType> = Some(ValueType::Object);
    const DATA: Option<DataType> = T::KIND;
}
impl<T: Data> FixType for Ref<T> {
    const VALUE: Option<ValueType> = Some(ValueType::Ref);
    const DATA: Option<DataType> = T::KIND;
}
impl FixType for Thunk {
    const VALUE: Option<ValueType> = Some(ValueType::Thunk);
}
impl FixType for Encode {
    const VALUE: Option<ValueType> = Some(ValueType::Encode);
}
