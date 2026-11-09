use super::*;
pub enum Any {}
pub enum Blob {}
pub enum Tree {}
pub struct Object<T = Any>(PhantomData<T>);
pub struct Ref<T = Any>(PhantomData<T>);
/// An Object or a Ref.
pub struct Evaluated<T = Any>(PhantomData<T>);
pub enum Thunk {}
pub enum Encode {}

/// An explicit evaluation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Evaluation {
    /// The argument is passed as given.
    Lazy,
    /// A Thunk is evaluated by a shallow Encode and an Object becomes its Ref.
    Shallow,
    /// A Thunk is evaluated by a strict Encode; Refs and Objects are kept.
    Comparable,
    /// The argument is evaluated to an Object.
    Strict,
}

pub trait FixType {
    /// The value types this type admits; empty admits all.
    const VALUES: &'static [ValueType];
    const DATA: Option<DataType> = None;
}
pub trait Data {
    const KIND: Option<DataType>;
}
impl FixType for Any {
    const VALUES: &'static [ValueType] = &[];
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
    const VALUES: &'static [ValueType] = &[ValueType::Object];
    const DATA: Option<DataType> = T::KIND;
}
impl<T: Data> FixType for Ref<T> {
    const VALUES: &'static [ValueType] = &[ValueType::Ref];
    const DATA: Option<DataType> = T::KIND;
}
impl<T: Data> FixType for Evaluated<T> {
    const VALUES: &'static [ValueType] = &[ValueType::Object, ValueType::Ref];
    const DATA: Option<DataType> = T::KIND;
}
impl FixType for Thunk {
    const VALUES: &'static [ValueType] = &[ValueType::Thunk];
}
impl FixType for Encode {
    const VALUES: &'static [ValueType] = &[ValueType::Encode];
}
