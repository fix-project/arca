#![no_std]

pub mod associated {
    pub type Runtime<T> = <T as super::Value>::Runtime;
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
    pub type AnyValue<T> = <Runtime<T> as super::Runtime>::AnyValue;
    pub type DynValue<T> = super::DynValue<Runtime<T>>;
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
    type AnyValue: AnyValue<Runtime = Self>;

    fn create_null(&self) -> Self::Null;
    fn create_word(&self, value: u64) -> Self::Word;
    fn create_error(&self, value: Self::AnyValue) -> Self::Error;
    fn create_atom(&self, data: &[u8]) -> Self::Atom;
    fn create_blob(&self, data: &[u8]) -> Self::Blob;
    fn create_tree(&self, values: &mut [Self::AnyValue]) -> Self::Tree;
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

pub trait Value: Sized + Clone + Into<associated::DynValue<Self>> {
    type Runtime: Runtime;
}

pub trait Null: Value {}

pub trait Word: Value {
    fn read(&self) -> u64;
}

pub trait Error: Value {
    fn read(self) -> associated::AnyValue<Self>;
}

pub trait Atom: Value + Eq {}

pub trait Blob: Value {
    fn read(&self, buffer: &mut [u8]);

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Tree: Value {
    fn read(&self, buffer: &mut [associated::AnyValue<Self>]);

    fn take(&mut self, index: usize) -> associated::AnyValue<Self>;

    fn put(
        &mut self,
        index: usize,
        value: associated::AnyValue<Self>,
    ) -> associated::AnyValue<Self>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Page: Value {
    fn read(&self, offset: usize, buffer: &mut [u8]);
    fn write(&mut self, offset: usize, buffer: &[u8]);

    fn size(&self) -> usize;
}

pub trait Table: Value {
    fn take(&mut self, index: usize) -> Entry<Self>;
    fn put(&mut self, offset: usize, entry: Entry<Self>) -> Result<Entry<Self>, ()>;

    fn size(&self) -> usize;
}

pub trait Lambda: Value {
    fn apply(self, argument: associated::AnyValue<Self>) -> associated::Thunk<Self>;
    fn read(self) -> (associated::Thunk<Self>, usize);
}

pub trait Thunk: Value {
    fn run(self) -> associated::AnyValue<Self>;
    fn read(
        self,
    ) -> (
        associated::Blob<Self>,
        associated::Table<Self>,
        associated::Tree<Self>,
    );
}

pub trait AnyValue: Value {}

#[derive(Clone, Debug)]
pub enum DynValue<R: Runtime> {
    Null(R::Null),
    Word(R::Word),
    Error(R::Error),
    Atom(R::Atom),
    Blob(R::Blob),
    Tree(R::Tree),
    Page(R::Page),
    Table(R::Table),
    Lambda(R::Lambda),
    Thunk(R::Thunk),
}

#[derive(Clone)]
pub enum Entry<T: Table> {
    Null(associated::Null<T>),
    ROPage(associated::Page<T>),
    RWPage(associated::Page<T>),
    ROTable(associated::Table<T>),
    RWTable(associated::Table<T>),
}
