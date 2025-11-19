use super::prelude::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value<R: Runtime> {
    Null(Null<R>),
    Word(Word<R>),
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
            Value::Blob(_) => DataType::Blob,
            Value::Tuple(_) => DataType::Tuple,
            Value::Page(_) => DataType::Page,
            Value::Table(_) => DataType::Table,
            Value::Function(_) => DataType::Function,
        }
    }

    pub fn byte_size(&self) -> usize {
        match self {
            Value::Null(_) => 0,
            Value::Word(word) => word.len(),
            Value::Blob(blob) => blob.len(),
            Value::Tuple(tuple) => tuple
                .iter()
                .map(|v| v.byte_size())
                .reduce(|x, y| x + y)
                .unwrap(),
            Value::Page(page) => page.len(),
            Value::Table(table) => table
                .iter()
                .map(|e| e.byte_size())
                .reduce(|x, y| x + y)
                .unwrap(),
            Value::Function(function) => function.read_cloned().byte_size(),
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

impl<R: Runtime> From<usize> for Value<R> {
    fn from(value: usize) -> Self {
        Value::Word((value as u64).into())
    }
}

impl<R: Runtime> From<i32> for Value<R> {
    fn from(value: i32) -> Self {
        Value::Word((value as u64).into())
    }
}

impl<R: Runtime> From<u32> for Value<R> {
    fn from(value: u32) -> Self {
        Value::Word((value as u64).into())
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ValueRef<'a, R: Runtime> {
    Null(&'a Null<R>),
    Word(&'a Word<R>),
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
            ValueRef::Blob(x) => RawValueRef::Blob(x.inner()),
            ValueRef::Tuple(x) => RawValueRef::Tuple(x.inner()),
            ValueRef::Page(x) => RawValueRef::Page(x.inner()),
            ValueRef::Table(x) => RawValueRef::Table(x.inner()),
            ValueRef::Function(x) => RawValueRef::Function(x.inner()),
        }
    }

    pub fn byte_size(&self) -> usize {
        match self {
            ValueRef::Null(_) => 0,
            ValueRef::Word(word) => word.len(),
            ValueRef::Blob(blob) => blob.len(),
            ValueRef::Tuple(tuple) => tuple
                .iter()
                .map(|v| v.byte_size())
                .reduce(|x, y| x + y)
                .unwrap(),
            ValueRef::Page(page) => page.len(),
            ValueRef::Table(table) => table
                .iter()
                .map(|e| e.byte_size())
                .reduce(|x, y| x + y)
                .unwrap(),
            ValueRef::Function(function) => function.read_cloned().byte_size(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RawValueRef<'a, R: Runtime> {
    Null(&'a R::Null),
    Word(&'a R::Word),
    Blob(&'a R::Blob),
    Tuple(&'a R::Tuple),
    Page(&'a R::Page),
    Table(&'a R::Table),
    Function(&'a R::Function),
}
