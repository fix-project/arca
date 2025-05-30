use crate::prelude::*;

pub mod arca;
pub mod atom;
pub mod lambda;
pub mod page;
pub mod pagetable;
pub mod thunk;

pub use arca::Arca;
pub use arca::LoadedArca;
pub use atom::Atom;
use common::message::Handle;
use common::message::Type;
pub use page::DynPage;
pub use pagetable::PageTable;
pub type Blob = Arc<[u8]>;
pub type Tree = Arc<[Value]>;
pub use lambda::Lambda;
pub use lambda::LoadedLambda;
pub use thunk::LoadedThunk;
pub use thunk::Thunk;

#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub enum Value {
    #[default]
    Null,
    Error(Arc<Value>),
    Word(u64),
    Atom(Arc<Atom>),
    Blob(Blob),
    Tree(Tree),
    Page(Arc<DynPage>),
    PageTable(Arc<PageTable>),
    Lambda(Lambda),
    Thunk(Thunk),
}

pub type LoadedTree<'a> = Vec<LoadedValue<'a>>;
#[derive(Debug)]
pub enum LoadedValue<'a> {
    Unloaded(Value),
    Error(Box<LoadedValue<'a>>),
    Tree(LoadedTree<'a>),
    Lambda(LoadedLambda<'a>),
    Thunk(LoadedThunk<'a>),
}

impl LoadedValue<'_> {
    pub fn unload(self) -> Value {
        match self {
            LoadedValue::Unloaded(x) => x,
            LoadedValue::Error(loaded_value) => Value::Error(loaded_value.unload().into()),
            LoadedValue::Tree(t) => Value::Tree(t.into_iter().map(|x| x.unload()).collect()),
            LoadedValue::Lambda(loaded_lambda) => Value::Lambda(loaded_lambda.unload()),
            LoadedValue::Thunk(loaded_thunk) => Value::Thunk(loaded_thunk.unload()),
        }
    }
}

impl From<LoadedValue<'_>> for Value {
    fn from(value: LoadedValue) -> Self {
        value.unload()
    }
}

impl From<Handle> for Value {
    fn from(value: Handle) -> Self {
        unsafe {
            match value.datatype {
                Type::Null => Value::Null,
                Type::Error => Value::Error(Arc::from_raw(value.parts[0] as _)),
                Type::Word => Value::Word(value.get_word().unwrap()),
                Type::Atom => Value::Atom(Arc::from_raw(value.parts[0] as _)),
                Type::Blob => Value::Blob(Arc::from_raw(core::ptr::from_raw_parts(
                    value.parts[0] as *const u8,
                    value.parts[1],
                ))),
                Type::Tree => Value::Tree(Arc::from_raw(core::ptr::from_raw_parts(
                    value.parts[0] as *const Value,
                    value.parts[1],
                ))),
                Type::Page => Value::Page(Arc::from_raw(value.parts[0] as _)),
                Type::PageTable => Value::PageTable(Arc::from_raw(value.parts[0] as _)),
                Type::Lambda => Value::Lambda(*Box::from_raw(value.parts[0] as _)),
                Type::Thunk => Value::Thunk(*Box::from_raw(value.parts[0] as _)),
            }
        }
    }
}

impl From<Value> for Handle {
    fn from(value: Value) -> Self {
        match value {
            Value::Null => Handle::null(),
            Value::Error(value) => Handle {
                parts: [Arc::into_raw(value) as usize, 0],
                datatype: Type::Error,
            },
            Value::Word(value) => Handle::word(value),
            Value::Atom(value) => Handle {
                parts: [Arc::into_raw(value) as usize, 0],
                datatype: Type::Atom,
            },
            Value::Blob(value) => {
                let parts = Arc::into_raw(value).to_raw_parts();
                unsafe { Handle::blob(parts.0 as usize, parts.1) }
            }
            Value::Tree(value) => {
                let parts = Arc::into_raw(value).to_raw_parts();
                Handle {
                    parts: [parts.0 as usize, parts.1],
                    datatype: Type::Tree,
                }
            }
            Value::Page(value) => Handle {
                parts: [Arc::into_raw(value) as usize, 0],
                datatype: Type::Page,
            },
            Value::PageTable(value) => Handle {
                parts: [Arc::into_raw(value) as usize, 0],
                datatype: Type::PageTable,
            },
            Value::Lambda(value) => Handle {
                parts: [Box::into_raw(value.into()) as usize, 0],
                datatype: Type::Lambda,
            },
            Value::Thunk(value) => Handle {
                parts: [Box::into_raw(value.into()) as usize, 0],
                datatype: Type::Thunk,
            },
        }
    }
}
