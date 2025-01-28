use alloc::{string::String, sync::Arc};

pub mod arca;
pub mod lambda;
pub mod page;
pub mod pagetable;
pub mod thunk;

pub use arca::Arca;
pub use page::Page;
pub use pagetable::PageTable;
pub type Blob = Arc<[u8]>;
pub type Tree = Arc<[Value]>;
pub use lambda::Lambda;
pub use thunk::Thunk;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Value {
    Null,
    Error(Arc<Value>),
    Atom(String),
    Blob(Blob),
    Tree(Tree),
    Page(Page),
    PageTable(PageTable),
    Lambda(Lambda),
    Thunk(Thunk),
}
