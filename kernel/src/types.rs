use crate::prelude::*;
use ::arca::ValueType;
use common::message::Handle;

pub mod arca;
pub mod atom;
pub mod blob;
pub mod error;
pub mod lambda;
pub mod null;
pub mod page;
pub mod runtime;
pub mod table;
pub mod thunk;
pub mod tree;
pub mod word;

pub use arca::Arca;
pub use arca::LoadedArca;
pub use atom::Atom;
pub use blob::Blob;
pub use error::Error;
pub use lambda::Lambda;
pub use null::Null;
pub use page::Page;
pub use runtime::Runtime;
pub use table::{Entry, Table};
pub use thunk::Thunk;
pub use tree::Tree;
pub use word::Word;

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Word(Word),
    Atom(Atom),
    Error(Error),
    Blob(Blob),
    Tree(Tree),
    Page(Page),
    Table(Table),
    Lambda(Lambda),
    Thunk(Thunk),
}

impl ::arca::RuntimeType for Value {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl ::arca::Value for Value {
    fn datatype(&self) -> DataType {
        match self {
            Value::Null => DataType::Null,
            Value::Word(_) => DataType::Word,
            Value::Atom(_) => DataType::Atom,
            Value::Error(_) => DataType::Error,
            Value::Blob(_) => DataType::Blob,
            Value::Tree(_) => DataType::Tree,
            Value::Page(_) => DataType::Page,
            Value::Table(_) => DataType::Table,
            Value::Lambda(_) => DataType::Lambda,
            Value::Thunk(_) => DataType::Thunk,
        }
    }
}

impl From<Null> for Value {
    fn from(_: Null) -> Self {
        Value::Null
    }
}

impl From<Word> for Value {
    fn from(value: Word) -> Self {
        Value::Word(value)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Value::Word(value.into())
    }
}

impl From<Atom> for Value {
    fn from(value: Atom) -> Self {
        Value::Atom(value)
    }
}

impl From<Error> for Value {
    fn from(value: Error) -> Self {
        Value::Error(value)
    }
}

impl From<Blob> for Value {
    fn from(value: Blob) -> Self {
        Value::Blob(value)
    }
}

impl From<Tree> for Value {
    fn from(value: Tree) -> Self {
        Value::Tree(value)
    }
}

impl From<Page> for Value {
    fn from(value: Page) -> Self {
        Value::Page(value)
    }
}

impl From<Table> for Value {
    fn from(value: Table) -> Self {
        Value::Table(value)
    }
}

impl From<Lambda> for Value {
    fn from(value: Lambda) -> Self {
        Value::Lambda(value)
    }
}

impl From<Thunk> for Value {
    fn from(value: Thunk) -> Self {
        Value::Thunk(value)
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct TypeError {
    expected: DataType,
    got: Value,
}

impl TryFrom<Value> for Null {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Null = value {
            Ok(Null::new())
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Word {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Word(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Atom {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Atom(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Error {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Error(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Blob {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Blob(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Tree {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Tree(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Page {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Page(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Table {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Table(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Lambda {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Lambda(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl TryFrom<Value> for Thunk {
    type Error = TypeError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Thunk(x) = value {
            Ok(x)
        } else {
            Err(TypeError {
                expected: <Self as ValueType>::DATATYPE,
                got: value,
            })
        }
    }
}

impl From<Handle> for Value {
    fn from(value: Handle) -> Self {
        match value.datatype() {
            DataType::Null => Value::Null,
            DataType::Word => Value::Word(Word::try_from(value).unwrap()),
            DataType::Error => Value::Error(Error::try_from(value).unwrap()),
            DataType::Atom => Value::Atom(Atom::try_from(value).unwrap()),
            DataType::Blob => Value::Blob(Blob::try_from(value).unwrap()),
            DataType::Tree => Value::Tree(Tree::try_from(value).unwrap()),
            DataType::Page => Value::Page(Page::try_from(value).unwrap()),
            DataType::Table => Value::Table(Table::try_from(value).unwrap()),
            DataType::Lambda => Value::Lambda(Lambda::try_from(value).unwrap()),
            DataType::Thunk => Value::Thunk(Thunk::try_from(value).unwrap()),
        }
    }
}

impl From<Value> for Handle {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Null.into(),
            Value::Word(value) => value.into(),
            Value::Error(value) => value.into(),
            Value::Atom(value) => value.into(),
            Value::Blob(value) => value.into(),
            Value::Tree(value) => value.into(),
            Value::Page(value) => value.into(),
            Value::Table(value) => value.into(),
            Value::Lambda(value) => value.into(),
            Value::Thunk(value) => value.into(),
        }
    }
}
