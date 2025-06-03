use crate::prelude::*;
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
pub use table::Table;
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

impl TryFrom<Value> for Null {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Null = value {
            Ok(Null::new())
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Word {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Word(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Atom {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Atom(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Error {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Error(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Blob {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Blob(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Tree {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Tree(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Page {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Page(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Table {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Table(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Lambda {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Lambda(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl TryFrom<Value> for Thunk {
    type Error = Value;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if let Value::Thunk(x) = value {
            Ok(x)
        } else {
            Err(value)
        }
    }
}

impl From<Handle> for Value {
    fn from(_value: Handle) -> Self {
        todo!();
        // unsafe {
        //     match value.datatype {
        //         Type::Null => Value::Null,
        //         Type::Error => Value::Error(Arc::from_raw(value.parts[0] as _)),
        //         Type::Word => Value::Word(value.get_word().unwrap()),
        //         Type::Atom => Value::Atom(Arc::from_raw(value.parts[0] as _)),
        //         Type::Blob => Value::Blob(Arc::from_raw(core::ptr::from_raw_parts(
        //             value.parts[0] as *const u8,
        //             value.parts[1],
        //         ))),
        //         Type::Tree => Value::Tree(Arc::from_raw(core::ptr::from_raw_parts(
        //             value.parts[0] as *const Value,
        //             value.parts[1],
        //         ))),
        //         Type::Page => Value::Page(Arc::from_raw(value.parts[0] as _)),
        //         Type::PageTable => Value::PageTable(Arc::from_raw(value.parts[0] as _)),
        //         Type::Lambda => Value::Lambda(*Box::from_raw(value.parts[0] as _)),
        //         Type::Thunk => Value::Thunk(*Box::from_raw(value.parts[0] as _)),
        //     }
        // }
    }
}

impl From<Value> for Handle {
    fn from(_value: Value) -> Self {
        todo!();
        // match value {
        //     Value::Null => Handle::null(),
        //     Value::Error(value) => Handle {
        //         parts: [Arc::into_raw(value) as usize, 0],
        //         datatype: Type::Error,
        //     },
        //     Value::Word(value) => Handle::word(value),
        //     Value::Atom(value) => Handle {
        //         parts: [Arc::into_raw(value) as usize, 0],
        //         datatype: Type::Atom,
        //     },
        //     Value::Blob(value) => {
        //         let parts = Arc::into_raw(value).to_raw_parts();
        //         unsafe { Handle::blob(parts.0 as usize, parts.1) }
        //     }
        //     Value::Tree(value) => {
        //         let parts = Arc::into_raw(value).to_raw_parts();
        //         Handle {
        //             parts: [parts.0 as usize, parts.1],
        //             datatype: Type::Tree,
        //         }
        //     }
        //     Value::Page(value) => Handle {
        //         parts: [Arc::into_raw(value) as usize, 0],
        //         datatype: Type::Page,
        //     },
        //     Value::PageTable(value) => Handle {
        //         parts: [Arc::into_raw(value) as usize, 0],
        //         datatype: Type::PageTable,
        //     },
        //     Value::Lambda(value) => Handle {
        //         parts: [Box::into_raw(value.into()) as usize, 0],
        //         datatype: Type::Lambda,
        //     },
        //     Value::Thunk(value) => Handle {
        //         parts: [Box::into_raw(value.into()) as usize, 0],
        //         datatype: Type::Thunk,
        //     },
        // }
    }
}
