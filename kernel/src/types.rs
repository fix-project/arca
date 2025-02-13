use alloc::boxed::Box;
use alloc::vec::Vec;
use alloc::{string::String, sync::Arc};

pub mod arca;
pub mod lambda;
pub mod page;
pub mod pagetable;
pub mod thunk;

pub use arca::Arca;
pub use arca::LoadedArca;
pub use page::Page;
pub use pagetable::PageTable;
pub type Blob = Arc<[u8]>;
pub type Tree = Arc<[Value]>;
pub use lambda::Lambda;
pub use lambda::LoadedLambda;
pub use thunk::LoadedThunk;
pub use thunk::Thunk;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Null,
    Error(Arc<Value>),
    Atom(String),
    Blob(Blob),
    Tree(Tree),
    Page(Arc<Page>),
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
