use crate::{DataType, EncodeType, FixShell, Invocation, ValueType};
use core::{
    fmt,
    ops::{Deref, Range},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeError {
    WrongType,
}
impl fmt::Display for TypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("wrong Fix type")
    }
}
impl core::error::Error for TypeError {}

#[repr(transparent)]
pub struct Value<S: FixShell>(S::Value);

impl<S: FixShell> Value<S> {
    pub fn from_inner(value: S::Value) -> Self {
        Self(value)
    }
    pub fn inner(&self) -> &S::Value {
        &self.0
    }
    pub fn into_inner(self) -> S::Value {
        self.0
    }
    pub fn value_type(&self) -> ValueType {
        S::value_type(&self.0)
    }
    pub fn is_comparable(&self) -> bool {
        S::is_comparable(&self.0)
    }
    pub fn equals(&self, other: &Self) -> Result<bool, S::Error> {
        S::equals(&self.0, &other.0)
    }
    pub fn eval(&self) -> Result<Self, S::Error> {
        S::eval(&self.0).map(Self)
    }
    pub fn advise(&self, key: &str, payload: &[u8]) {
        S::advise(&self.0, key, payload)
    }
    pub fn effect(&self) -> Self
    where
        S: Invocation,
    {
        Self(S::effect(&self.0))
    }
}
impl<S: FixShell> Default for Value<S> {
    fn default() -> Self {
        Self(S::blob(&[]))
    }
}
impl<S: FixShell> PartialEq for Value<S> {
    fn eq(&self, other: &Self) -> bool {
        self.is_comparable() && other.is_comparable() && self.equals(other).unwrap_or(false)
    }
}
impl<S: FixShell> Clone for Value<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<S: FixShell> fmt::Debug for Value<S>
where
    S::Value: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<S: FixShell> AsRef<Value<S>> for Value<S> {
    fn as_ref(&self) -> &Self {
        self
    }
}

macro_rules! typed {
    ($name:ident, $kind:expr, $data:expr) => {
        #[repr(transparent)]
        pub struct $name<S: FixShell>(Value<S>);
        impl<S: FixShell> $name<S> {
            pub fn as_value(&self) -> &Value<S> {
                &self.0
            }
            pub fn into_value(self) -> Value<S> {
                self.0
            }
            pub fn into_inner(self) -> S::Value {
                self.0.into_inner()
            }
        }
        impl<S: FixShell> PartialEq for $name<S> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl<S: FixShell> From<&$name<S>> for $name<S> {
            fn from(value: &$name<S>) -> Self {
                value.clone()
            }
        }
        impl<S: FixShell> From<&$name<S>> for Value<S> {
            fn from(value: &$name<S>) -> Self {
                value.0.clone()
            }
        }
        impl<S: FixShell> Clone for $name<S> {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }
        impl<S: FixShell> fmt::Debug for $name<S>
        where
            S::Value: fmt::Debug,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl<S: FixShell> From<$name<S>> for Value<S> {
            fn from(value: $name<S>) -> Self {
                value.0
            }
        }
        impl<S: FixShell> Deref for $name<S> {
            type Target = Value<S>;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
        impl<S: FixShell> AsRef<Value<S>> for $name<S> {
            fn as_ref(&self) -> &Value<S> {
                &self.0
            }
        }
        impl<S: FixShell> TryFrom<Value<S>> for $name<S> {
            type Error = TypeError;
            fn try_from(value: Value<S>) -> Result<Self, Self::Error> {
                if S::value_type(value.inner()) != $kind {
                    return Err(TypeError::WrongType);
                }
                let data: Option<DataType> = $data;
                if let Some(data) = data {
                    if S::data_type(value.inner()) != data {
                        return Err(TypeError::WrongType);
                    }
                }
                Ok(Self(value))
            }
        }
        impl<S: FixShell> TryFrom<&Value<S>> for $name<S> {
            type Error = TypeError;
            fn try_from(value: &Value<S>) -> Result<Self, Self::Error> {
                Self::try_from(value.clone())
            }
        }
    };
}
typed!(Object, ValueType::Object, None);
typed!(Blob, ValueType::Object, Some(DataType::Blob));
typed!(Tree, ValueType::Object, Some(DataType::Tree));
typed!(Reference, ValueType::Ref, None);
typed!(Thunk, ValueType::Thunk, None);
typed!(Encode, ValueType::Encode, None);

impl<S: FixShell> Default for Blob<S> {
    fn default() -> Self {
        Self::new(&[])
    }
}
impl<S: FixShell> Eq for Blob<S> {}
impl<S: FixShell> Default for Tree<S> {
    fn default() -> Self {
        Self::new(&[])
    }
}
impl<S: FixShell> Default for Object<S> {
    fn default() -> Self {
        Blob::default().into()
    }
}

impl<S: FixShell> Blob<S> {
    pub fn new(bytes: &[u8]) -> Self {
        Self(Value(S::blob(bytes)))
    }
    pub fn len(&self) -> usize {
        S::len(self.0.inner())
            .try_into()
            .expect("value exceeds address space")
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn read(&self, offset: usize, bytes: &mut [u8]) -> Result<usize, S::Error> {
        let length = bytes.len().min(self.len().saturating_sub(offset));
        if length != 0 {
            S::read(self.0.inner(), offset, &mut bytes[..length])?;
        }
        Ok(length)
    }
    pub fn read_exact(&self, offset: usize, bytes: &mut [u8]) -> Result<(), S::Error> {
        if bytes.is_empty() {
            return Ok(());
        }
        S::read(self.0.inner(), offset, bytes)
    }
    pub fn reference(&self) -> Reference<S> {
        Reference(Value(S::reference(self.0.inner())))
    }
    #[cfg(feature = "alloc")]
    pub fn to_vec(&self) -> Result<alloc::vec::Vec<u8>, S::Error> {
        let length = self.len();
        let mut bytes = alloc::vec![0; length];
        self.read_exact(0, &mut bytes)?;
        Ok(bytes)
    }
}

impl<S: FixShell> Tree<S> {
    pub fn new(children: &[Value<S>]) -> Self {
        let children = unsafe {
            // Value is transparent over S::Value.
            core::slice::from_raw_parts(children.as_ptr().cast::<S::Value>(), children.len())
        };
        Self(Value(S::tree(children)))
    }
    pub fn len(&self) -> usize {
        S::len(self.0.inner())
            .try_into()
            .expect("value exceeds address space")
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Result<Option<Value<S>>, S::Error> {
        if index >= self.len() {
            return Ok(None);
        }
        S::get(self.0.inner(), index).map(|value| Some(Value(value)))
    }
    pub fn iter(&self) -> Iter<S, &Self> {
        Iter {
            shell: core::marker::PhantomData,
            tree: self,
            indices: 0..self.len(),
        }
    }
    pub fn is_tag(&self) -> bool {
        S::is_tag(self.0.inner())
    }
    pub fn reference(&self) -> Reference<S> {
        Reference(Value(S::reference(self.0.inner())))
    }
    pub fn application(&self) -> Thunk<S> {
        Thunk(Value(S::application(self.0.inner())))
    }
    pub fn selection(&self) -> Thunk<S> {
        Thunk(Value(S::selection(self.0.inner())))
    }
    pub fn tag(fields: &[Value<S>]) -> Result<Self, S::Error>
    where
        S: Invocation,
    {
        let fields = unsafe {
            core::slice::from_raw_parts(fields.as_ptr().cast::<S::Value>(), fields.len())
        };
        S::tag(fields).map(|value| Self(Value(value)))
    }
}

impl<S: FixShell> Object<S> {
    pub fn data_type(&self) -> DataType {
        S::data_type(self.0.inner())
    }
    pub fn reference(&self) -> Reference<S> {
        Reference(Value(S::reference(self.0.inner())))
    }
}
macro_rules! object {
    ($($name:ident),*) => {$(
        impl<S: FixShell> From<$name<S>> for Object<S> { fn from(value: $name<S>) -> Self { Self(value.0) } }
    )*};
}
object!(Blob, Tree);
impl<S: FixShell> Reference<S> {
    pub fn data_type(&self) -> DataType {
        S::data_type(self.0.inner())
    }
    pub fn len(&self) -> usize {
        S::len(self.0.inner())
            .try_into()
            .expect("value exceeds address space")
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn identification(&self) -> Thunk<S> {
        Thunk(Value(S::identification(self.0.inner())))
    }
}
impl<S: FixShell> Thunk<S> {
    pub fn strict(&self) -> Encode<S> {
        Encode(Value(S::strict(self.0.inner())))
    }
    pub fn shallow(&self) -> Encode<S> {
        Encode(Value(S::shallow(self.0.inner())))
    }
}
impl<S: FixShell> Encode<S> {
    pub fn encode_type(&self) -> EncodeType {
        S::encode_type(self.0.inner())
    }
    pub fn eval(&self) -> Result<Value<S>, S::Error> {
        self.0.eval()
    }
}
macro_rules! scalar {
    ($($ty:ty),*) => {$(
        impl<S: FixShell> From<$ty> for Blob<S> {
            fn from(value: $ty) -> Self { Self::new(&value.to_le_bytes()) }
        }
    )*};
}
scalar!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);
impl<S: FixShell> From<&[u8]> for Blob<S> {
    fn from(bytes: &[u8]) -> Self {
        Self::new(bytes)
    }
}

pub struct Iter<S: FixShell, T: core::borrow::Borrow<Tree<S>>> {
    tree: T,
    shell: core::marker::PhantomData<fn() -> S>,
    indices: Range<usize>,
}
impl<S: FixShell, T: core::borrow::Borrow<Tree<S>> + Clone> Clone for Iter<S, T> {
    fn clone(&self) -> Self {
        Self {
            tree: self.tree.clone(),
            indices: self.indices.clone(),
            shell: core::marker::PhantomData,
        }
    }
}
impl<S: FixShell, T: core::borrow::Borrow<Tree<S>>> fmt::Debug for Iter<S, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Iter")
            .field("indices", &self.indices)
            .finish_non_exhaustive()
    }
}
impl<S: FixShell, T: core::borrow::Borrow<Tree<S>>> Iterator for Iter<S, T> {
    type Item = Result<Value<S>, S::Error>;
    fn next(&mut self) -> Option<Self::Item> {
        let index = self.indices.next()?;
        Some(S::get(self.tree.borrow().inner(), index).map(Value))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}
impl<S: FixShell, T: core::borrow::Borrow<Tree<S>>> DoubleEndedIterator for Iter<S, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let index = self.indices.next_back()?;
        Some(S::get(self.tree.borrow().inner(), index).map(Value))
    }
}
impl<S: FixShell, T: core::borrow::Borrow<Tree<S>>> ExactSizeIterator for Iter<S, T> {}
impl<S: FixShell, T: core::borrow::Borrow<Tree<S>>> core::iter::FusedIterator for Iter<S, T> {}
impl<S: FixShell> IntoIterator for Tree<S> {
    type Item = Result<Value<S>, S::Error>;
    type IntoIter = Iter<S, Self>;
    fn into_iter(self) -> Self::IntoIter {
        let indices = 0..self.len();
        Iter {
            shell: core::marker::PhantomData,
            tree: self,
            indices,
        }
    }
}
impl<S: FixShell> IntoIterator for &Tree<S> {
    type Item = Result<Value<S>, S::Error>;
    type IntoIter = Iter<S, Self>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<S: FixShell> From<&str> for Blob<S> {
    fn from(value: &str) -> Self {
        Self::new(value.as_bytes())
    }
}
impl<S: FixShell, const N: usize> From<[u8; N]> for Blob<S> {
    fn from(value: [u8; N]) -> Self {
        Self::new(&value)
    }
}

impl<S: FixShell> From<bool> for Blob<S> {
    fn from(value: bool) -> Self {
        Self::new(&[u8::from(value)])
    }
}
impl<S: FixShell, const N: usize> From<[Value<S>; N]> for Tree<S> {
    fn from(values: [Value<S>; N]) -> Self {
        Self::new(&values)
    }
}
impl<S: FixShell> From<&[Value<S>]> for Tree<S> {
    fn from(values: &[Value<S>]) -> Self {
        Self::new(values)
    }
}
#[cfg(feature = "alloc")]
impl<S: FixShell> FromIterator<Value<S>> for Tree<S> {
    fn from_iter<I: IntoIterator<Item = Value<S>>>(values: I) -> Self {
        let values: alloc::vec::Vec<_> = values.into_iter().collect();
        Self::new(&values)
    }
}
#[cfg(feature = "alloc")]
impl<S: FixShell> From<alloc::vec::Vec<u8>> for Blob<S> {
    fn from(value: alloc::vec::Vec<u8>) -> Self {
        Self::new(&value)
    }
}
#[cfg(feature = "alloc")]
impl<S: FixShell> From<alloc::string::String> for Blob<S> {
    fn from(value: alloc::string::String) -> Self {
        Self::from(value.as_str())
    }
}

#[cfg(feature = "alloc")]
impl<S: FixShell> From<alloc::vec::Vec<Value<S>>> for Tree<S> {
    fn from(values: alloc::vec::Vec<Value<S>>) -> Self {
        Self::new(&values)
    }
}
