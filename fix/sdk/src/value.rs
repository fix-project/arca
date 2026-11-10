use crate::{DataType, EncodeType, Error, ThunkType, ValueType, ffi};
use core::marker::PhantomData;
mod types;
pub use types::*;

#[repr(transparent)]
pub struct Handle<T = Any> {
    index: u32,
    kind: PhantomData<T>,
}

impl<T> Handle<T> {
    /// # Safety
    /// `index` must be an open descriptor whose ownership is transferred here.
    pub unsafe fn from_raw(index: u32) -> Self {
        Self {
            index,
            kind: PhantomData,
        }
    }
    pub fn as_raw(&self) -> u32 {
        self.index
    }
    pub fn into_raw(self) -> u32 {
        let index = self.index;
        core::mem::forget(self);
        index
    }
    pub(crate) fn cast<U>(self) -> Handle<U> {
        unsafe { Handle::from_raw(self.into_raw()) }
    }
}
impl<T> core::fmt::Debug for Handle<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Handle").field(&self.index).finish()
    }
}
impl<T> Drop for Handle<T> {
    fn drop(&mut self) {
        unsafe { ffi::close(self.index) }
    }
}
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        unsafe { Self::from_raw(ffi::clone(self.index)) }
    }
}
impl<T: FixType> Handle<T> {
    pub fn checked<U: FixType>(self) -> Result<Handle<U>, Error> {
        if !U::VALUES.is_empty() && !U::VALUES.contains(&self.value_type()?) {
            return Err(Error::WrongType);
        }
        if let Some(kind) = U::DATA
            && self.data_type()? != kind
        {
            return Err(Error::WrongType);
        }
        Ok(self.cast())
    }
}
macro_rules! narrowing {
    ($($target:ty),*) => {$(
        impl TryFrom<Handle<Any>> for Handle<$target> {
            type Error = Error;
            fn try_from(value: Handle<Any>) -> Result<Self, Error> { value.checked() }
        }
        impl TryFrom<&Handle<Any>> for Handle<$target> {
            type Error = Error;
            fn try_from(value: &Handle<Any>) -> Result<Self, Error> { value.clone().checked() }
        }
        impl From<Handle<$target>> for Handle<Any> {
            fn from(value: Handle<$target>) -> Self { value.cast() }
        }
    )*};
}
narrowing!(
    Object<Any>,
    Object<Blob>,
    Object<Tree>,
    Ref<Any>,
    Ref<Blob>,
    Ref<Tree>,
    Evaluated<Any>,
    Evaluated<Blob>,
    Evaluated<Tree>,
    Thunk,
    Encode
);

pub enum RawHandle<'a, T> {
    Borrowed(&'a Handle<T>),
    Owned(Handle<T>),
}
impl<T> RawHandle<'_, T> {
    pub fn as_raw(&self) -> u32 {
        match self {
            Self::Borrowed(handle) => handle.as_raw(),
            Self::Owned(handle) => handle.as_raw(),
        }
    }
    fn into_owned(self) -> Handle<T> {
        match self {
            Self::Borrowed(handle) => handle.clone(),
            Self::Owned(handle) => handle,
        }
    }
}

pub trait Value {
    type Type: FixType;
    fn handle(&self) -> RawHandle<'_, Self::Type>;
    fn to_handle(&self) -> Handle<Self::Type> {
        self.handle().into_owned()
    }
    fn capture(&self) -> Result<Handle<Any>, Error> {
        Ok(self.to_handle().cast())
    }
    fn erase(&self) -> Handle<Any> {
        self.to_handle().cast()
    }
    fn value_type(&self) -> Result<ValueType, Error> {
        Ok(match unsafe { ffi::get_type(self.handle().as_raw()) } {
            0 => ValueType::Object,
            1 => ValueType::Ref,
            2 => ValueType::Thunk,
            3 => ValueType::Encode,
            _ => unreachable!(),
        })
    }
    fn data_type(&self) -> Result<DataType, Error> {
        if !matches!(self.value_type()?, ValueType::Object | ValueType::Ref) {
            return Err(Error::WrongType);
        }
        Ok(
            match unsafe { ffi::get_data_type(self.handle().as_raw()) } {
                0 => DataType::Blob,
                1 => DataType::Tree,
                _ => unreachable!(),
            },
        )
    }
    fn encode_type(&self) -> Result<EncodeType, Error> {
        if self.value_type()? != ValueType::Encode {
            return Err(Error::WrongType);
        }
        Ok(
            match unsafe { ffi::get_encode_type(self.handle().as_raw()) } {
                0 => EncodeType::Strict,
                1 => EncodeType::Shallow,
                _ => unreachable!(),
            },
        )
    }
    fn unsound_thunk_type(&self) -> Result<ThunkType, Error> {
        Ok(
            match unsafe { ffi::get_thunk_type(self.handle().as_raw()) } {
                0 => ThunkType::Application,
                1 => ThunkType::Selection,
                2 => ThunkType::Identification,
                _ => unreachable!(),
            },
        )
    }
    fn len(&self) -> Result<u64, Error> {
        if !matches!(self.value_type()?, ValueType::Object | ValueType::Ref) {
            return Err(Error::WrongType);
        }
        Ok(unsafe { ffi::len(self.handle().as_raw()) })
    }
    fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }
    fn is_comparable(&self) -> Result<bool, Error> {
        Ok(unsafe { ffi::is_comparable(self.handle().as_raw()) } != 0)
    }
    fn is_value(&self) -> Result<bool, Error> {
        Ok(unsafe { ffi::is_value(self.handle().as_raw()) } != 0)
    }
    fn is_tag(&self) -> Result<bool, Error> {
        Ok(unsafe { ffi::is_tag(self.handle().as_raw()) } != 0)
    }
    fn is_strict(&self) -> Result<bool, Error> {
        Ok(self.encode_type()? == EncodeType::Strict)
    }
    fn is_shallow(&self) -> Result<bool, Error> {
        Ok(self.encode_type()? == EncodeType::Shallow)
    }
    fn equals<V: Value>(&self, other: V) -> Result<bool, Error>
    where
        Self: Sized,
    {
        if !self.is_comparable()? || !other.is_comparable()? {
            return Err(Error::NotComparable);
        }
        Ok(unsafe { ffi::equals(self.handle().as_raw(), other.handle().as_raw()) } != 0)
    }
    fn unpack<T>(&self) -> Result<T, Error>
    where
        Self: Sized,
        T: TryFrom<Handle<Object<Tree>>, Error = Error>,
    {
        self.capture()?.checked::<Object<Tree>>()?.try_into()
    }
    fn with<F, V>(&self, index: usize, function: F) -> Result<V, Error>
    where
        Self: Sized,
        F: FnOnce(Handle<Any>) -> Result<V, Error>,
    {
        function(self.get(index))
    }
    fn get(&self, index: usize) -> Handle<Any> {
        let mut descriptor = 0u32;
        let count = unsafe {
            ffi::access_tree(
                self.handle().as_raw(),
                index.try_into().expect("tree index"),
                &mut descriptor as *mut _ as u32,
                1,
            )
        };
        assert_eq!(count, 1, "tree index out of bounds");
        unsafe { Handle::from_raw(descriptor) }
    }
    fn reference(&self) -> Handle<Ref> {
        unsafe { Handle::from_raw(ffi::create_ref(self.handle().as_raw())) }
    }
    fn identification(&self) -> Handle<Thunk> {
        unsafe { Handle::from_raw(ffi::create_identification_thunk(self.handle().as_raw())) }
    }
    fn application(&self) -> Handle<Thunk> {
        unsafe { Handle::from_raw(ffi::create_application_thunk(self.handle().as_raw())) }
    }
    fn selection(&self) -> Handle<Thunk> {
        unsafe { Handle::from_raw(ffi::create_selection_thunk(self.handle().as_raw())) }
    }
    fn strict(&self) -> Handle<Encode> {
        unsafe { Handle::from_raw(ffi::create_strict_encode(self.handle().as_raw())) }
    }
    fn shallow(&self) -> Handle<Encode> {
        unsafe { Handle::from_raw(ffi::create_shallow_encode(self.handle().as_raw())) }
    }
    fn tag(&self) -> Result<Handle<Object<Tree>>, Error> {
        if self.value_type()? != ValueType::Object || self.data_type()? != DataType::Tree {
            return Err(Error::WrongType);
        }
        if self.len()? == 0 {
            return Err(Error::OutOfBounds);
        }
        if !self.get(0).is_comparable()? {
            return Err(Error::NotComparable);
        }
        Ok(unsafe { Handle::from_raw(ffi::create_tag(self.handle().as_raw())) })
    }
    fn tag_unchecked(&self) -> Result<Handle<Object<Tree>>, Error> {
        Ok(unsafe { Handle::from_raw(ffi::create_tag_unchecked(self.handle().as_raw())) })
    }
    fn eval(&self) -> Result<Handle<Any>, Error> {
        Ok(unsafe { Handle::from_raw(ffi::eval(self.handle().as_raw())) })
    }
    fn effect(&self) -> Result<Handle<Any>, Error> {
        Ok(unsafe { Handle::from_raw(ffi::effect(self.handle().as_raw())) })
    }
    fn return_cc(&self) -> Result<Handle<Any>, Error> {
        Ok(unsafe { Handle::from_raw(ffi::return_cc(self.handle().as_raw())) })
    }
    fn call_cc(&self) -> Result<Handle<Any>, Error> {
        Ok(unsafe { Handle::from_raw(ffi::call_cc(self.handle().as_raw())) })
    }
    fn unsound_handle_bytes(&self) -> Result<Handle<Object<Blob>>, Error> {
        Ok(unsafe { Handle::from_raw(ffi::unsound_handle_bytes(self.handle().as_raw())) })
    }
    fn raw_handle_bytes(&self) -> Result<[u8; 32], Error> {
        let mut bytes = [0; 32];
        unsafe { ffi::read_handle_bytes(self.handle().as_raw(), bytes.as_mut_ptr() as u32) };
        Ok(bytes)
    }
    fn read_into(&self, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        if self.value_type()? != ValueType::Object || self.data_type()? != DataType::Blob {
            return Err(Error::WrongType);
        }
        let length = usize::try_from(self.len()?)?;
        if offset > length || bytes.len() > length - offset {
            return Err(Error::OutOfBounds);
        }
        let count = unsafe {
            ffi::access_blob(
                self.handle().as_raw(),
                offset.try_into()?,
                bytes.as_mut_ptr() as u32,
                bytes.len().try_into()?,
            )
        };
        if count as usize != bytes.len() {
            return Err(Error::OutOfBounds);
        }
        Ok(())
    }
    fn read_u64(&self) -> Result<u64, Error> {
        if self.len()? != 8 {
            return Err(Error::WrongType);
        }
        let mut bytes = [0; 8];
        self.read_into(0, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
    #[cfg(feature = "alloc")]
    fn read(&self) -> Result<alloc::vec::Vec<u8>, Error> {
        let mut bytes = alloc::vec![0; self.len()?.try_into()?];
        self.read_into(0, &mut bytes)?;
        Ok(bytes)
    }
}
impl<T: FixType> Value for Handle<T> {
    type Type = T;
    fn handle(&self) -> RawHandle<'_, T> {
        RawHandle::Borrowed(self)
    }
}
impl<V: Value + ?Sized> Value for &V {
    type Type = V::Type;
    fn handle(&self) -> RawHandle<'_, Self::Type> {
        (**self).handle()
    }
}
impl<V: Value> Value for Result<V, Error> {
    type Type = V::Type;
    fn handle(&self) -> RawHandle<'_, Self::Type> {
        self.as_ref().expect("Fix value").handle()
    }
}
#[cfg(feature = "alloc")]
impl<V: Value + ?Sized> Value for alloc::rc::Rc<V> {
    type Type = V::Type;
    fn handle(&self) -> RawHandle<'_, Self::Type> {
        (**self).handle()
    }
}
impl<T: FixType, V: Value> PartialEq<V> for Handle<T> {
    fn eq(&self, rhs: &V) -> bool {
        self.equals(rhs).unwrap_or(false)
    }
}

impl Handle<Object<Tree>> {
    pub fn iter(&self) -> Result<impl Iterator<Item = Handle<Any>> + '_, Error> {
        Ok((0..self.len()? as usize).map(|i| self.get(i)))
    }
}

impl Blob {
    pub fn create(bytes: impl AsRef<[u8]>) -> Handle<Object<Blob>> {
        let bytes = bytes.as_ref();
        unsafe {
            let pages = (bytes.len() as u32).div_ceil(65536);
            let size = memory_size_1();
            if pages > size {
                assert_ne!(
                    memory_grow_1(pages - size),
                    u32::MAX,
                    "blob allocation failed"
                );
            }
            memory_copy_0_to_1(0, bytes.as_ptr() as u32, bytes.len() as u32);
            Handle::from_raw(ffi::create_blob(1, bytes.len() as u32))
        }
    }
}
impl Tree {
    #[cfg(feature = "alloc")]
    pub fn tag(children: impl IntoIterator<Item: Value>) -> Result<Handle<Object<Tree>>, Error> {
        Self::from_iter(children).tag()
    }

    pub fn create<T: FixType>(children: impl AsRef<[Handle<T>]>) -> Handle<Object<Tree>> {
        let children = children.as_ref();
        unsafe {
            Handle::from_raw(ffi::create_tree(
                children.as_ptr() as u32,
                children.len() as u32,
            ))
        }
    }
    #[cfg(feature = "alloc")]
    #[allow(clippy::should_implement_trait)]
    pub fn from_iter<V: Value>(children: impl IntoIterator<Item = V>) -> Handle<Object<Tree>> {
        let children: alloc::vec::Vec<_> = children.into_iter().map(|v| v.erase()).collect();
        Self::create(children)
    }
}
macro_rules! scalar {
    ($($ty:ty),*) => {$(
        impl From<$ty> for Handle<Object<Blob>> { fn from(value:$ty)->Self { Blob::create(value.to_le_bytes()) } }
        impl Value for $ty { type Type=Object<Blob>; fn handle(&self)->RawHandle<'_,Self::Type> { RawHandle::Owned((*self).into()) } }
    )*};
}
scalar!(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64);
impl From<bool> for Handle<Object<Blob>> {
    fn from(value: bool) -> Self {
        Blob::create([value as u8])
    }
}
impl Value for bool {
    type Type = Object<Blob>;
    fn handle(&self) -> RawHandle<'_, Self::Type> {
        RawHandle::Owned((*self).into())
    }
}
impl From<&[u8]> for Handle<Object<Blob>> {
    fn from(value: &[u8]) -> Self {
        Blob::create(value)
    }
}
impl<const N: usize> From<[u8; N]> for Handle<Object<Blob>> {
    fn from(value: [u8; N]) -> Self {
        Blob::create(value)
    }
}

#[macro_export]
macro_rules! tree { ($($child:expr),* $(,)?) => {{ let children: &[$crate::Handle] = &[$($crate::Value::erase(&$child)),*]; $crate::Tree::create(children) }}; }

#[link(wasm_import_module = "__wasm_postprocessor")]
unsafe extern "C" {
    fn memory_size_1() -> u32;
    fn memory_grow_1(pages: u32) -> u32;
    fn memory_copy_0_to_1(destination: u32, source: u32, length: u32);
}

pub fn choose<V: Value>(condition: bool, left: V, right: V) -> V {
    if condition { left } else { right }
}

pub enum Choice<L, R> {
    Left(L),
    Right(R),
}
impl<L: Value, R: Value> Value for Choice<L, R> {
    type Type = Any;
    fn handle(&self) -> RawHandle<'_, Any> {
        RawHandle::Owned(match self {
            Self::Left(value) => value.erase(),
            Self::Right(value) => value.erase(),
        })
    }
}
impl<T: FixType> Handle<T> {
    pub fn into_any(self) -> Handle<Any> {
        self.cast()
    }
}
#[cfg(feature = "alloc")]
impl<T: FixType> TryFrom<alloc::rc::Rc<dyn Value<Type = Any> + '_>> for Handle<T> {
    type Error = Error;
    fn try_from(value: alloc::rc::Rc<dyn Value<Type = Any> + '_>) -> Result<Self, Error> {
        value.capture()?.checked()
    }
}
impl<T: FixType> TryFrom<&dyn Value<Type = Any>> for Handle<T> {
    type Error = Error;
    fn try_from(value: &dyn Value<Type = Any>) -> Result<Self, Error> {
        value.capture()?.checked()
    }
}
impl Handle<Object<Tree>> {
    pub fn coordinate(&self, name: &[u8]) -> Result<(u64, Handle<Any>), Error> {
        let mut status = 0u64;
        let value = unsafe {
            ffi::coordinate(
                name.as_ptr() as u32,
                name.len().try_into()?,
                self.as_raw(),
                &mut status as *mut _ as u32,
            )
        };
        Ok((status, unsafe { Handle::from_raw(value) }))
    }
}

impl<T> From<&Handle<T>> for Handle<T> {
    fn from(value: &Handle<T>) -> Self {
        value.clone()
    }
}
impl<V: Value + ?Sized> Value for &mut V {
    type Type = V::Type;
    fn handle(&self) -> RawHandle<'_, Self::Type> {
        (**self).handle()
    }
}

impl From<&str> for Handle<Object<Blob>> {
    fn from(value: &str) -> Self {
        Blob::create(value.as_bytes())
    }
}
#[cfg(feature = "alloc")]
impl From<alloc::vec::Vec<u8>> for Handle<Object<Blob>> {
    fn from(value: alloc::vec::Vec<u8>) -> Self {
        Blob::create(value)
    }
}
#[cfg(feature = "alloc")]
impl From<alloc::string::String> for Handle<Object<Blob>> {
    fn from(value: alloc::string::String) -> Self {
        Blob::create(value.as_bytes())
    }
}
impl<const N: usize> From<[Handle<Any>; N]> for Handle<Object<Tree>> {
    fn from(value: [Handle<Any>; N]) -> Self {
        Tree::create(value)
    }
}

macro_rules! tuples {
    ($($value:ident : $index:tt),+) => {
        impl<$($value:Value),+> From<($($value,)+)> for Handle<Object<Tree>> {
            fn from(values:($($value,)+))->Self {
                Tree::create([$(values.$index.erase()),+])
            }
        }
        impl<$($value:FixType),+> TryFrom<Handle<Object<Tree>>> for ($(Handle<$value>,)+) {
            type Error=Error;
            fn try_from(value:Handle<Object<Tree>>)->Result<Self,Error> {
                let length=[$(stringify!($value)),+].len();
                if value.len()?!=length as u64 {return Err(Error::WrongType);}
                Ok(($(value.get($index).checked()?,)+))
            }
        }
    };
}
tuples!(A:0);
tuples!(A:0,B:1);
tuples!(A:0,B:1,C:2);
tuples!(A:0,B:1,C:2,D:3);

impl Value for fix::Value<crate::backend::Shell> {
    type Type = Any;
    fn handle(&self) -> RawHandle<'_, Any> {
        RawHandle::Borrowed(self.inner())
    }
}
macro_rules! sdk_value {
    ($($wrapper:ident : $kind:ty),* $(,)?) => {$(
        impl Value for fix::$wrapper<crate::backend::Shell> {
            type Type = $kind;
            fn handle(&self) -> RawHandle<'_, Self::Type> {
                RawHandle::Owned(self.as_value().inner().clone().cast())
            }
        }
    )*};
}
sdk_value!(Blob: Object<Blob>, Tree: Object<Tree>, Object: Object<Any>,
    Reference: Ref<Any>, Thunk: Thunk, Encode: Encode);
