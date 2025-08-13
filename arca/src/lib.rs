#![feature(unboxed_closures)]
#![feature(fn_traits)]
#![no_std]

use core::ops::{Deref, DerefMut};

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "serde")]
mod serde;

pub mod blob;
pub mod datatype;
pub mod entry;
pub mod function;
pub mod null;
pub mod page;
pub mod runtime;
pub mod table;
pub mod tuple;
pub mod value;
pub mod word;

pub mod prelude {
    pub use super::{
        Blob, Function, Null, Page, Table, Tuple, Word, datatype::DataType, entry::Entry,
        function::Continuation, runtime::Runtime, value::RawValue, value::RawValueRef,
        value::Value, value::ValueRef,
    };
    #[cfg(feature = "alloc")]
    pub(crate) use alloc::{vec, vec::Vec};
}

pub use prelude::*;

macro_rules! foreach_type_item {
    ($e:ident) => {
        $e! {Null}
        $e! {Word}
        $e! {Blob}
        $e! {Tuple}
        $e! {Page}
        $e! {Table}
        $e! {Function}
    };
}

macro_rules! decl_runtime_type {
    ($x:ident) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq)]
        pub struct $x<R: Runtime>(R::$x);

        impl<R: Runtime> $x<R> {
            pub fn from_inner(data: R::$x) -> Self {
                Self(data)
            }

            pub fn inner(&self) -> &R::$x {
                &self.0
            }

            pub fn inner_mut(&mut self) -> &mut R::$x {
                &mut self.0
            }

            pub fn into_inner(self) -> R::$x {
                self.0
            }

            pub fn len(&self) -> usize {
                R::value_len(ValueRef::$x(self))
            }

            #[must_use]
            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }
        }
    };
}

foreach_type_item! {decl_runtime_type}

macro_rules! impl_deref {
    ($x:ident) => {
        impl<R: Runtime> Deref for $x<R>
        where
            R::$x: Deref,
        {
            type Target = <R::$x as Deref>::Target;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<R: Runtime> DerefMut for $x<R>
        where
            R::$x: DerefMut,
        {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}

impl_deref!(Word);
impl_deref!(Blob);
impl_deref!(Page);

macro_rules! impl_tryfrom_value {
    ($x:ident) => {
        impl<R: Runtime> TryFrom<Value<R>> for $x<R> {
            type Error = Value<R>;

            fn try_from(value: Value<R>) -> Result<Self, Self::Error> {
                if let Value::$x(x) = value {
                    Ok(x)
                } else {
                    Err(value)
                }
            }
        }
    };
}

foreach_type_item! {impl_tryfrom_value}

macro_rules! impl_value_from {
    ($x:ident) => {
        impl<R: Runtime> From<$x<R>> for Value<R> {
            fn from(value: $x<R>) -> Self {
                Value::$x(value)
            }
        }
    };
}

foreach_type_item! {impl_value_from}
