#![no_std]

pub mod associated {
    pub type Runtime<T> = <T as super::RuntimeType>::Runtime;
    pub type Null<T> = <Runtime<T> as super::Runtime>::Null;
    pub type Word<T> = <Runtime<T> as super::Runtime>::Word;
    pub type Error<T> = <Runtime<T> as super::Runtime>::Error;
    pub type Atom<T> = <Runtime<T> as super::Runtime>::Atom;
    pub type Blob<T> = <Runtime<T> as super::Runtime>::Blob;
    pub type Tree<T> = <Runtime<T> as super::Runtime>::Tree;
    pub type Page<T> = <Runtime<T> as super::Runtime>::Page;
    pub type Table<T> = <Runtime<T> as super::Runtime>::Table;
    pub type Lambda<T> = <Runtime<T> as super::Runtime>::Lambda;
    pub type Thunk<T> = <Runtime<T> as super::Runtime>::Thunk;
    pub type Value<T> = <Runtime<T> as super::Runtime>::Value;
}

pub trait Runtime: Sized {
    type Null: Null<Runtime = Self>;
    type Word: Word<Runtime = Self>;
    type Error: Error<Runtime = Self>;
    type Atom: Atom<Runtime = Self>;
    type Blob: Blob<Runtime = Self>;
    type Tree: Tree<Runtime = Self>;
    type Page: Page<Runtime = Self>;
    type Table: Table<Runtime = Self>;
    type Lambda: Lambda<Runtime = Self>;
    type Thunk: Thunk<Runtime = Self>;
    type Value: Value<Runtime = Self>;

    fn create_null(&self) -> Self::Null;
    fn create_word(&self, value: u64) -> Self::Word;
    fn create_error(&self, value: Self::Value) -> Self::Error;
    fn create_atom(&self, data: &[u8]) -> Self::Atom;
    fn create_blob(&self, data: &[u8]) -> Self::Blob;
    fn create_tree(&self, size: usize) -> Self::Tree;
    fn create_page(&self, size: usize) -> Self::Page;
    fn create_table(&self, size: usize) -> Self::Table;
    fn create_lambda(&self, thunk: Self::Thunk, index: usize) -> Self::Lambda;
    fn create_thunk(
        &self,
        registers: Self::Blob,
        memory: Self::Table,
        descriptors: Self::Tree,
    ) -> Self::Thunk;
}

pub trait RuntimeType: Sized + Clone {
    type Runtime: Runtime;
}

pub trait ValueType: RuntimeType {
    const DATATYPE: DataType;
}

pub trait Null: ValueType {}

pub trait Word: ValueType {
    fn read(&self) -> u64;
}

pub trait Error: ValueType {
    fn read(self) -> associated::Value<Self>;
}

pub trait Atom: ValueType + Eq {}

pub trait Blob: ValueType {
    fn read(&self, buffer: &mut [u8]);

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Tree: ValueType {
    fn take(&mut self, index: usize) -> associated::Value<Self>;

    fn put(&mut self, index: usize, value: associated::Value<Self>) -> associated::Value<Self>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Page: ValueType {
    fn read(&self, offset: usize, buffer: &mut [u8]);
    fn write(&mut self, offset: usize, buffer: &[u8]);

    fn size(&self) -> usize;
}

pub trait Table: ValueType {
    fn take(&mut self, index: usize) -> Entry<Self>;
    fn put(&mut self, index: usize, entry: Entry<Self>) -> Result<Entry<Self>, Entry<Self>>;

    fn size(&self) -> usize;
}

pub trait Lambda: ValueType {
    fn apply(self, argument: associated::Value<Self>) -> associated::Thunk<Self>;
    fn read(self) -> (associated::Thunk<Self>, usize);
}

pub trait Thunk: ValueType {
    fn run(self) -> associated::Value<Self>;
    fn read(
        self,
    ) -> (
        associated::Blob<Self>,
        associated::Table<Self>,
        associated::Tree<Self>,
    );
}

pub trait Value:
    RuntimeType
    + From<associated::Null<Self>>
    + From<associated::Word<Self>>
    + From<associated::Error<Self>>
    + From<associated::Atom<Self>>
    + From<associated::Blob<Self>>
    + From<associated::Tree<Self>>
    + From<associated::Page<Self>>
    + From<associated::Table<Self>>
    + From<associated::Lambda<Self>>
    + From<associated::Thunk<Self>>
    + TryInto<associated::Null<Self>>
    + TryInto<associated::Word<Self>>
    + TryInto<associated::Error<Self>>
    + TryInto<associated::Atom<Self>>
    + TryInto<associated::Blob<Self>>
    + TryInto<associated::Tree<Self>>
    + TryInto<associated::Page<Self>>
    + TryInto<associated::Table<Self>>
    + TryInto<associated::Lambda<Self>>
    + TryInto<associated::Thunk<Self>>
{
    fn datatype(&self) -> DataType;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DataType {
    Null,
    Word,
    Error,
    Atom,
    Blob,
    Tree,
    Page,
    Table,
    Lambda,
    Thunk,
}

#[derive(Clone)]
pub enum Entry<T: Table> {
    Null(associated::Null<T>),
    ROPage(associated::Page<T>),
    RWPage(associated::Page<T>),
    ROTable(associated::Table<T>),
    RWTable(associated::Table<T>),
}

impl<T: Table> Entry<T> {
    pub fn size(&self) -> usize {
        match self {
            Entry::Null(_) => 0,
            Entry::ROPage(page) => page.size(),
            Entry::RWPage(page) => page.size(),
            Entry::ROTable(table) => table.size(),
            Entry::RWTable(table) => table.size(),
        }
    }
}
