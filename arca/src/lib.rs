#![feature(unboxed_closures)]
#![feature(fn_traits)]
#![no_std]

use core::ops::{Deref, DerefMut};

pub mod prelude {
    pub use super::{
        Atom, Blob, DataType, Entry, Exception, Function, Null, Page, Runtime as _, Table, Tuple,
        Value, ValueRef, Word,
    };
}

pub trait Runtime: Sized + core::fmt::Debug + Eq + PartialEq + Clone {
    type Null: Clone + core::fmt::Debug + Eq + PartialEq;
    type Word: Clone + core::fmt::Debug + Eq + PartialEq;
    type Exception: Clone + core::fmt::Debug + Eq + PartialEq;
    type Atom: Clone + core::fmt::Debug + Eq + PartialEq;
    type Blob: Clone + core::fmt::Debug + Eq + PartialEq;
    type Tuple: Clone + core::fmt::Debug + Eq + PartialEq;
    type Page: Clone + core::fmt::Debug + Eq + PartialEq;
    type Table: Clone + core::fmt::Debug + Eq + PartialEq;
    type Function: Clone + core::fmt::Debug + Eq + PartialEq;

    type Error: core::fmt::Debug;

    fn create_null() -> Null<Self>;
    fn create_word(word: u64) -> Word<Self>;
    fn create_exception(value: Value<Self>) -> Exception<Self>;
    fn create_atom(bytes: &[u8]) -> Atom<Self>;
    fn create_blob(bytes: &[u8]) -> Blob<Self>;
    fn create_tuple(len: usize) -> Tuple<Self>;
    fn create_page(len: usize) -> Page<Self>;
    fn create_table(len: usize) -> Table<Self>;
    fn create_function(arca: bool, data: Value<Self>) -> Result<Function<Self>, Self::Error>;

    fn value_len(value: ValueRef<Self>) -> usize;

    fn read_word(word: &Word<Self>) -> u64;
    fn read_exception(exception: Exception<Self>) -> Value<Self>;
    fn read_blob(blob: &Blob<Self>, offset: usize, buf: &mut [u8]) -> usize;
    fn read_page(page: &Page<Self>, offset: usize, buf: &mut [u8]) -> usize;

    fn write_blob(blob: &mut Blob<Self>, offset: usize, buf: &[u8]) -> usize;
    fn write_page(page: &mut Page<Self>, offset: usize, buf: &[u8]) -> usize;

    fn get_tuple(tuple: &Tuple<Self>, index: usize) -> Result<Value<Self>, Self::Error>;
    fn set_tuple(
        tuple: &mut Tuple<Self>,
        index: usize,
        value: Value<Self>,
    ) -> Result<Value<Self>, Self::Error>;

    fn get_table(table: &Table<Self>, index: usize) -> Result<Entry<Self>, Self::Error>;
    fn set_table(
        table: &mut Table<Self>,
        index: usize,
        entry: Entry<Self>,
    ) -> Result<Entry<Self>, Self::Error>;

    fn apply_function(function: Function<Self>, argument: Value<Self>) -> Function<Self>;
    fn force_function(function: Function<Self>) -> Value<Self>;
    fn is_function_arcane(function: &Function<Self>) -> bool;
    fn call_with_current_continuation(function: Function<Self>) -> Value<Self>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value<R: Runtime> {
    Null(Null<R>),
    Word(Word<R>),
    Exception(Exception<R>),
    Atom(Atom<R>),
    Blob(Blob<R>),
    Tuple(Tuple<R>),
    Page(Page<R>),
    Table(Table<R>),
    Function(Function<R>),
}

impl<R: Runtime> Default for Value<R> {
    fn default() -> Self {
        Value::Null(Null::default())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawValue<R: Runtime> {
    Null(R::Null),
    Word(R::Word),
    Exception(R::Exception),
    Atom(R::Atom),
    Blob(R::Blob),
    Tuple(R::Tuple),
    Page(R::Page),
    Table(R::Table),
    Function(R::Function),
}

impl<R: Runtime> Value<R> {
    pub fn into_inner(self) -> RawValue<R> {
        match self {
            Value::Null(x) => RawValue::Null(x.into_inner()),
            Value::Word(x) => RawValue::Word(x.into_inner()),
            Value::Exception(x) => RawValue::Exception(x.into_inner()),
            Value::Atom(x) => RawValue::Atom(x.into_inner()),
            Value::Blob(x) => RawValue::Blob(x.into_inner()),
            Value::Tuple(x) => RawValue::Tuple(x.into_inner()),
            Value::Page(x) => RawValue::Page(x.into_inner()),
            Value::Table(x) => RawValue::Table(x.into_inner()),
            Value::Function(x) => RawValue::Function(x.into_inner()),
        }
    }

    pub fn datatype(&self) -> DataType {
        match self {
            Value::Null(_) => DataType::Null,
            Value::Word(_) => DataType::Word,
            Value::Exception(_) => DataType::Exception,
            Value::Atom(_) => DataType::Atom,
            Value::Blob(_) => DataType::Blob,
            Value::Tuple(_) => DataType::Tuple,
            Value::Page(_) => DataType::Page,
            Value::Table(_) => DataType::Table,
            Value::Function(_) => DataType::Function,
        }
    }
}

impl<R: Runtime> From<Option<Value<R>>> for Value<R> {
    fn from(value: Option<Value<R>>) -> Self {
        value.unwrap_or_default()
    }
}

impl<R: Runtime> From<u64> for Value<R> {
    fn from(value: u64) -> Self {
        Value::Word(value.into())
    }
}

impl<R: Runtime> From<&[u8]> for Value<R> {
    fn from(value: &[u8]) -> Self {
        Value::Blob(value.into())
    }
}

impl<R: Runtime> From<&str> for Value<R> {
    fn from(value: &str) -> Self {
        Value::Blob(value.into())
    }
}

macro_rules! foreach_type_item {
    ($e:ident) => {
        $e! {Null}
        $e! {Word}
        $e! {Atom}
        $e! {Exception}
        $e! {Blob}
        $e! {Tuple}
        $e! {Page}
        $e! {Table}
        $e! {Function}
    };
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ValueRef<'a, R: Runtime> {
    Null(&'a Null<R>),
    Word(&'a Word<R>),
    Exception(&'a Exception<R>),
    Atom(&'a Atom<R>),
    Blob(&'a Blob<R>),
    Tuple(&'a Tuple<R>),
    Page(&'a Page<R>),
    Table(&'a Table<R>),
    Function(&'a Function<R>),
}

impl<'a, R: Runtime> ValueRef<'a, R> {
    pub fn inner(self) -> RawValueRef<'a, R> {
        match self {
            ValueRef::Null(x) => RawValueRef::Null(x.inner()),
            ValueRef::Word(x) => RawValueRef::Word(x.inner()),
            ValueRef::Exception(x) => RawValueRef::Exception(x.inner()),
            ValueRef::Atom(x) => RawValueRef::Atom(x.inner()),
            ValueRef::Blob(x) => RawValueRef::Blob(x.inner()),
            ValueRef::Tuple(x) => RawValueRef::Tuple(x.inner()),
            ValueRef::Page(x) => RawValueRef::Page(x.inner()),
            ValueRef::Table(x) => RawValueRef::Table(x.inner()),
            ValueRef::Function(x) => RawValueRef::Function(x.inner()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawValueRef<'a, R: Runtime> {
    Null(&'a R::Null),
    Word(&'a R::Word),
    Exception(&'a R::Exception),
    Atom(&'a R::Atom),
    Blob(&'a R::Blob),
    Tuple(&'a R::Tuple),
    Page(&'a R::Page),
    Table(&'a R::Table),
    Function(&'a R::Function),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DataType {
    Null,
    Word,
    Exception,
    Atom,
    Blob,
    Tuple,
    Page,
    Table,
    Function,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Entry<R: Runtime> {
    Null(usize),
    ROPage(Page<R>),
    RWPage(Page<R>),
    ROTable(Table<R>),
    RWTable(Table<R>),
}

impl<R: Runtime> Entry<R> {
    pub fn len(&self) -> usize {
        match self {
            Entry::Null(size) => *size,
            Entry::ROPage(page) => page.len(),
            Entry::RWPage(page) => page.len(),
            Entry::ROTable(table) => table.len(),
            Entry::RWTable(table) => table.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
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

impl<R: Runtime> Null<R> {
    pub fn new() -> Self {
        R::create_null()
    }
}

impl<R: Runtime> Default for Null<R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<R: Runtime> Word<R> {
    pub fn new(word: u64) -> Self {
        R::create_word(word)
    }

    pub fn read(&self) -> u64 {
        R::read_word(self)
    }
}

impl<R: Runtime> From<u64> for Word<R> {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl<R: Runtime> Exception<R> {
    pub fn new(value: impl Into<Value<R>>) -> Self {
        R::create_exception(value.into())
    }

    pub fn read(self) -> Value<R> {
        R::read_exception(self)
    }
}

impl<R: Runtime> Atom<R> {
    pub fn new(data: impl AsRef<[u8]>) -> Self {
        R::create_atom(data.as_ref())
    }
}

impl<R: Runtime> Blob<R> {
    pub fn new(data: impl AsRef<[u8]>) -> Self {
        R::create_blob(data.as_ref())
    }
}

impl<R: Runtime> From<&[u8]> for Blob<R> {
    fn from(value: &[u8]) -> Self {
        Self::new(value)
    }
}

impl<R: Runtime> From<&str> for Blob<R> {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl<R: Runtime> Tuple<R> {
    pub fn new(len: usize) -> Self {
        R::create_tuple(len)
    }

    pub fn get(&self, idx: usize) -> Value<R> {
        R::get_tuple(self, idx).unwrap()
    }

    pub fn set(&mut self, idx: usize, value: impl Into<Value<R>>) -> Value<R> {
        R::set_tuple(self, idx, value.into()).unwrap()
    }

    pub fn take(&mut self, idx: usize) -> Value<R> {
        let replacement = Value::default();
        self.set(idx, replacement)
    }

    pub fn swap(&mut self, idx: usize, value: &mut Value<R>) {
        let mut replacement = self.take(idx);
        core::mem::swap(&mut replacement, value);
        self.set(idx, replacement);
    }
}

impl<R: Runtime, A: Into<Value<R>>, B: Into<Value<R>>> From<(A, B)> for Tuple<R> {
    fn from(value: (A, B)) -> Self {
        let mut tuple = Tuple::new(2);
        tuple.set(0, value.0);
        tuple.set(1, value.1);
        tuple
    }
}

impl<R: Runtime, A: Into<Value<R>>, B: Into<Value<R>>, C: Into<Value<R>>> From<(A, B, C)>
    for Tuple<R>
{
    fn from(value: (A, B, C)) -> Self {
        let mut tuple = Tuple::new(3);
        tuple.set(0, value.0);
        tuple.set(1, value.1);
        tuple.set(2, value.2);
        tuple
    }
}

impl<R: Runtime, A: Into<Value<R>>, B: Into<Value<R>>, C: Into<Value<R>>, D: Into<Value<R>>>
    From<(A, B, C, D)> for Tuple<R>
{
    fn from(value: (A, B, C, D)) -> Self {
        let mut tuple = Tuple::new(4);
        tuple.set(0, value.0);
        tuple.set(1, value.1);
        tuple.set(2, value.2);
        tuple.set(3, value.3);
        tuple
    }
}

impl<R: Runtime> From<&mut [Value<R>]> for Tuple<R> {
    fn from(value: &mut [Value<R>]) -> Self {
        let mut tuple = Tuple::new(value.len());
        for (i, x) in value.iter_mut().enumerate() {
            let mut value = Value::default();
            core::mem::swap(&mut value, x);
            tuple.set(i, value);
        }
        tuple
    }
}

impl<R: Runtime> Page<R> {
    pub fn new(len: usize) -> Self {
        R::create_page(len)
    }

    pub fn read(&self, offset: usize, buf: &mut [u8]) -> usize {
        R::read_page(self, offset, buf)
    }

    pub fn write(&mut self, offset: usize, buf: &[u8]) -> usize {
        R::write_page(self, offset, buf)
    }
}

impl<R: Runtime> Table<R> {
    pub fn new(len: usize) -> Self {
        R::create_table(len)
    }

    pub fn get(&self, idx: usize) -> Result<Entry<R>, R::Error> {
        R::get_table(self, idx)
    }

    pub fn set(&mut self, idx: usize, entry: Entry<R>) -> Result<Entry<R>, R::Error> {
        R::set_table(self, idx, entry)
    }

    pub fn map(&mut self, address: usize, entry: Entry<R>) -> Result<Entry<R>, R::Error> {
        let result = if address + entry.len() >= self.len() {
            try_replace_with(self, |this: Self| -> Result<Self, R::Error> {
                let mut embiggened = R::create_table(this.len() * 512);
                embiggened.set(0, Entry::RWTable(this))?;
                Ok(embiggened)
            })?;
            self.map(address, entry)?
        } else if entry.len() == self.len() / 512 {
            let shift = entry.len().ilog2();
            let index = address >> shift;
            assert!(index < 512);
            self.set(index, entry)?
        } else {
            let shift = (self.len() / 512).ilog2();
            let index = (address >> shift) & 0x1ff;
            let offset = address & !(0x1ff << shift);

            let mut smaller = match self.set(index, Entry::Null(0))? {
                Entry::ROTable(table) => table,
                Entry::RWTable(table) => table,
                _ => R::create_table(self.len() / 512),
            };
            assert!(self.len() > smaller.len());
            smaller.map(offset, entry)?;
            self.set(index, Entry::RWTable(smaller))?
        };
        Ok(result)
    }

    pub fn unmap(&mut self, address: usize) -> Option<Entry<R>> {
        if address > self.len() {
            None
        } else {
            let shift = (self.len() / 512).ilog2();
            let index = (address >> shift) & 0x1ff;
            let offset = address & !(0x1ff << shift);

            let smaller = self.set(index, Entry::Null(0));
            match smaller.ok()? {
                Entry::ROTable(mut table) => {
                    let nested = table.unmap(offset);
                    assert!(self.set(index, Entry::ROTable(table)).is_ok());
                    nested
                }
                Entry::RWTable(mut table) => {
                    let nested = table.unmap(offset);
                    assert!(self.set(index, Entry::RWTable(table)).is_ok());
                    nested
                }
                x => Some(x),
            }
        }
    }
}

impl<R: Runtime> Function<R> {
    pub fn new(arca: bool, data: impl Into<Value<R>>) -> Result<Self, R::Error> {
        R::create_function(arca, data.into())
    }

    pub fn arcane(data: impl Into<Value<R>>) -> Result<Self, R::Error> {
        Self::new(true, data)
    }

    pub fn symbolic(value: impl Into<Value<R>>) -> Self {
        Self::new(false, value).unwrap()
    }

    pub fn apply(self, argument: impl Into<Value<R>>) -> Self {
        R::apply_function(self, argument.into())
    }

    pub fn force(self) -> Value<R> {
        R::force_function(self)
    }

    pub fn is_arcane(&self) -> bool {
        R::is_function_arcane(self)
    }

    pub fn is_symbolic(&self) -> bool {
        !self.is_arcane()
    }

    pub fn call_with_current_continuation(self) -> Value<R> {
        R::call_with_current_continuation(self)
    }
}

fn try_replace_with<T, E>(x: &mut T, f: impl FnOnce(T) -> Result<T, E>) -> Result<(), E> {
    unsafe {
        let old = core::ptr::read(x);
        let new = f(old)?;
        core::ptr::write(x, new);
        Ok(())
    }
}

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

#[derive(Copy, Clone, Debug, Default)]
pub struct Continuation;

impl<R: Runtime> FnOnce<(Continuation,)> for Function<R> {
    type Output = Value<R>;

    extern "rust-call" fn call_once(self, _: (Continuation,)) -> Self::Output {
        self.call_with_current_continuation()
    }
}

impl<R: Runtime, A: Into<Value<R>>> FnOnce<(A,)> for Function<R> {
    type Output = Function<R>;

    extern "rust-call" fn call_once(self, args: (A,)) -> Self::Output {
        self.apply(args.0.into())
    }
}

macro_rules! fn_impl {
    ($(($head:ident, $($rest:ident),+) => ($headf:tt, $($restf:tt),+)),+) => {
        $(
        impl<R: Runtime, $head, $($rest),+> FnOnce<($head, $($rest),+)> for Function<R>
        where
            Function<R>: FnOnce<($head,), Output = Function<R>>,
            Function<R>: FnOnce<($($rest),+,)>,
        {
            type Output = <Function<R> as FnOnce<($($rest),+,)>>::Output;

            extern "rust-call" fn call_once(self, args: ($head, $($rest),+)) -> <Function<R> as FnOnce<($($rest),+,)>>::Output {
                self(args.$headf)($(args.$restf),*)
            }
        }
        )*
    };
}

fn_impl! {
    (A, B) => (0, 1),
    (A, B, C) => (0, 1, 2),
    (A, B, C, D) => (0, 1, 2, 3),
    (A, B, C, D, E) => (0, 1, 2, 3, 4),
    (A, B, C, D, E, F) => (0, 1, 2, 3, 4, 5),
    (A, B, C, D, E, F, G) => (0, 1, 2, 3, 4, 5, 6)
}

#[derive(Debug, Clone)]
pub struct TupleIntoIter<R: Runtime> {
    tuple: Tuple<R>,
    len: usize,
    index: usize,
}

impl<R: Runtime> IntoIterator for Tuple<R> {
    type Item = Value<R>;

    type IntoIter = TupleIntoIter<R>;

    fn into_iter(self) -> Self::IntoIter {
        let len = self.len();
        TupleIntoIter {
            tuple: self,
            len,
            index: 0,
        }
    }
}

impl<R: Runtime> Iterator for TupleIntoIter<R> {
    type Item = Value<R>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }
        let next = self.tuple.take(self.index);
        self.index += 1;
        Some(next)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len - self.index, Some(self.len - self.index))
    }
}

impl<R: Runtime> ExactSizeIterator for TupleIntoIter<R> {}

impl<R: Runtime, V: Into<Value<R>>> FromIterator<V> for Tuple<R> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        let iter = iter.into_iter();
        let hint = iter.size_hint();
        let mut n = hint.0;
        let mut tuple: Tuple<R> = Tuple::new(n);
        let mut max_i = 0;
        for (i, x) in iter.enumerate() {
            if i < n {
                tuple.set(i, x);
                max_i = i;
            } else {
                let new_n = if let Some(upper) = hint.1 {
                    upper
                } else {
                    n * 2
                };
                let mut new_tuple = Tuple::new(new_n);
                for (i, x) in tuple.into_iter().enumerate() {
                    new_tuple.set(i, x);
                }
                n = new_n;
                tuple = new_tuple;
            }
        }
        if max_i < (n - 1) {
            let mut final_tuple = Tuple::new(max_i + 1);
            for (i, x) in tuple.into_iter().enumerate().take(max_i) {
                final_tuple.set(i, x);
            }
            tuple = final_tuple;
        }
        tuple
    }
}
