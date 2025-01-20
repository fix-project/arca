use alloc::sync::Arc;

pub mod arca;
pub mod page;
pub mod pagetable;

pub use arca::Arca;
pub use page::Page;
pub use pagetable::PageTable;
pub type Blob = Arc<[u8]>;
pub type Tree = Arc<[Value]>;

#[derive(Clone)]
pub enum Value {
    None,
    Blob(Blob),
    Tree(Tree),
    Page(Page),
    PageTable(PageTable),
    Function(Arca),
    Thunk(Arca, Arc<Value>),
}
