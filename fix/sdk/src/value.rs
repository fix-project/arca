use crate::{DataType, EncodeType, Error, ValueType};
use alloc::vec::Vec;
use core::marker::PhantomData;
macro_rules! delegate_value {
    (@call [$value:ident, {$($pattern:pat => $source:expr),+}], $method:ident, $arguments:tt) => {
        match $value { $($pattern => delegate_value!(@call ($source), $method, $arguments)),+ }
    };
    (@call ($value:expr), $method:ident, ($($argument:expr),*)) => {
        ($value).$method($($argument),*)
    };
    ($this:ident, match $value:ident { $($pattern:pat => $source:expr),+ $(,)? }) => {
        delegate_value!(@methods $this, path, [$value, {$($pattern => $source),+}], path);
    };
    ($this:ident, $value:expr) => {
        delegate_value!(@methods $this, path, ($value), path);
    };
    ($this:ident, $path:ident, $value:expr, $route:expr) => {
        delegate_value!(@methods $this, $path, ($value), $route);
    };
    (@methods $this:ident, $path:ident, $value:tt, $route:expr) => {
        fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, build_at, ($route))
        }
        fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, value_type_at, ($route))
        }
        fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, data_type_at, ($route))
        }
        fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, encode_type_at, ($route))
        }
        fn len_at(&self, path: Option<&Path<'_>>) -> Result<usize, Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, len_at, ($route))
        }
        fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, is_tag_at, ($route))
        }
        fn read_at(
            &self,
            path: Option<&Path<'_>>,
            offset: usize,
            bytes: &mut [u8],
        ) -> Result<(), Error> {
            let $this = self;
            let $path = path;
            delegate_value!(@call $value, read_at, ($route, offset, bytes))
        }
    };
}

mod adaptor;
mod blob;
mod cast;
mod encode;
mod ffi;
mod handle;
mod reference;
mod thunk;
mod tree;
mod types;

use adaptor::at_root;
pub use adaptor::{Choice, Erased, choose};
pub use blob::CreateBlob;
pub use cast::Cast;
use cast::checked;
pub use encode::{Shallow, Strict};
pub use ffi::__finish;
pub(crate) use ffi::status;
use ffi::*;
pub use handle::{Child, Focus, Handle, Lookup, Path};
pub use reference::Reference;
pub use thunk::{Application, Identification, Selection};
pub use tree::{Children, Cons, CreateTree, Nil, Select};
pub use types::*;

pub trait Value {
    type Type: FixType;
    #[doc(hidden)]
    fn build_at(&self, path: Option<&Path<'_>>) -> Result<(), Error>;
    #[doc(hidden)]
    fn value_type_at(&self, path: Option<&Path<'_>>) -> Result<ValueType, Error> {
        let _ = path;
        Err(Error::WrongType)
    }
    #[doc(hidden)]
    fn data_type_at(&self, path: Option<&Path<'_>>) -> Result<DataType, Error> {
        let _ = path;
        Err(Error::WrongType)
    }
    #[doc(hidden)]
    fn encode_type_at(&self, path: Option<&Path<'_>>) -> Result<EncodeType, Error> {
        let _ = path;
        Err(Error::WrongType)
    }
    #[doc(hidden)]
    fn len_at(&self, path: Option<&Path<'_>>) -> Result<usize, Error> {
        let _ = path;
        Err(Error::WrongType)
    }
    #[doc(hidden)]
    fn is_tag_at(&self, path: Option<&Path<'_>>) -> Result<bool, Error> {
        let _ = path;
        Err(Error::WrongType)
    }
    #[doc(hidden)]
    fn read_at(
        &self,
        path: Option<&Path<'_>>,
        offset: usize,
        bytes: &mut [u8],
    ) -> Result<(), Error> {
        let _ = (path, offset, bytes);
        Err(Error::WrongType)
    }
    fn unpack<T>(self) -> Result<T, Error>
    where
        Self: Sized + Value<Type = Object<Tree>>,
        T: TryFrom<Self, Error = Error>,
    {
        self.try_into()
    }
    fn with<F, V>(self, index: usize, function: F) -> Result<V, Error>
    where
        Self: Sized,
        F: FnOnce(Select<Self>) -> Result<V, Error>,
    {
        let child = Select {
            parent: self,
            index,
        };
        child.value_type()?;
        function(child)
    }
    fn erase(self) -> Erased<Self>
    where
        Self: Sized,
    {
        Erased(self)
    }
    fn value_type(&self) -> Result<ValueType, Error> {
        self.value_type_at(None)
    }
    fn data_type(&self) -> Result<DataType, Error> {
        self.data_type_at(None)
    }
    fn encode_type(&self) -> Result<EncodeType, Error> {
        self.encode_type_at(None)
    }
    fn len(&self) -> Result<usize, Error> {
        self.len_at(None)
    }
    fn is_empty(&self) -> Result<bool, Error> {
        Ok(self.len()? == 0)
    }
    fn is_eq(&self) -> Result<bool, Error> {
        self.build_at(None)?;
        Ok(query_current(fix_sdk_is_eq)? != 0)
    }
    fn equals<V: Value>(&self, other: V) -> Result<bool, Error>
    where
        Self: Sized,
    {
        unsafe extern "C" fn load<V: Value>(context: *const ()) -> u32 {
            let recipe = unsafe { &*context.cast::<V>() };
            code(recipe.build_at(None))
        }
        self.build_at(None)?;
        let mut out = 0;
        status(unsafe { fix_sdk_equals(load::<V>, (&other as *const V).cast(), &mut out) })?;
        Ok(out != 0)
    }
    fn reference<D: Data>(self) -> Reference<Self>
    where
        Self: Sized + Value<Type = Object<D>>,
    {
        Reference(self)
    }
    fn identification(self) -> Identification<Self>
    where
        Self: Sized,
    {
        Identification(self)
    }
    fn application(self) -> Application<Self>
    where
        Self: Sized,
    {
        Application(self)
    }
    fn selection(self) -> Selection<Self>
    where
        Self: Sized,
    {
        Selection(self)
    }
    fn strict(self) -> Strict<Self>
    where
        Self: Sized + Value<Type = Thunk>,
    {
        Strict(self)
    }
    fn shallow(self) -> Shallow<Self>
    where
        Self: Sized + Value<Type = Thunk>,
    {
        Shallow(self)
    }
    fn get(&self, index: usize) -> Select<&Self>
    where
        Self: Sized + Value<Type = Object<Tree>>,
    {
        Select {
            parent: self,
            index,
        }
    }
    fn read_into(&self, offset: usize, bytes: &mut [u8]) -> Result<(), Error> {
        self.read_at(None, offset, bytes)
    }
    fn read_u64(&self) -> Result<u64, Error> {
        if self.len()? != 8 {
            return Err(Error::WrongType);
        }
        let mut bytes = [0; 8];
        self.read_into(0, &mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
    fn read(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = alloc::vec![0; self.len()?];
        self.read_into(0, &mut bytes)?;
        Ok(bytes)
    }
    fn is_tag(&self) -> Result<bool, Error> {
        self.is_tag_at(None)
    }
    fn is_strict(&self) -> Result<bool, Error> {
        Ok(self.encode_type()? == EncodeType::Strict)
    }
    fn is_shallow(&self) -> Result<bool, Error> {
        Ok(self.encode_type()? == EncodeType::Shallow)
    }
}
impl<V: Value + ?Sized> Value for &V {
    type Type = V::Type;
    delegate_value!(this, *this);
}
impl<V: Value + ?Sized> Value for &mut V {
    type Type = V::Type;
    delegate_value!(this, &**this);
}
impl<V: Value + ?Sized> Value for alloc::rc::Rc<V> {
    type Type = V::Type;
    delegate_value!(this, &**this);
}
impl<V: Value> Value for Result<V, Error> {
    type Type = V::Type;
    delegate_value!(this, this.as_ref().map_err(|error| *error)?);
}
fn equal<L: Value, R: Value>(left: &L, right: &R) -> bool {
    match left.equals(right) {
        Err(Error::NotEq) => false,
        result => result.expect("Fix equality failed"),
    }
}
macro_rules! partial_eq {
    ($($adaptor:ident),* $(,)?) => {$ (
        impl<I, V: Value> PartialEq<V> for $adaptor<I> where Self: Value {
            fn eq(&self, other: &V) -> bool { equal(self, other) }
        }
    )*};
}
partial_eq!(
    CreateBlob,
    CreateTree,
    Select,
    Erased,
    Reference,
    Identification,
    Application,
    Selection,
    Strict,
    Shallow
);
impl<L, R, V: Value> PartialEq<V> for Choice<L, R>
where
    Self: Value,
{
    fn eq(&self, other: &V) -> bool {
        equal(self, other)
    }
}
impl<T: FixType, S: Value, V: Value> PartialEq<V> for Cast<T, S> {
    fn eq(&self, other: &V) -> bool {
        equal(self, other)
    }
}
