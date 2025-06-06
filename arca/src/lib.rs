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

    fn runtime(&self) -> &Self::Runtime;
}

pub trait ValueType: RuntimeType {
    const DATATYPE: DataType;
}

pub trait Null:
    ValueType + From<<Self::Runtime as Runtime>::Null> + Into<<Self::Runtime as Runtime>::Null>
{
}

pub trait Word:
    ValueType + From<<Self::Runtime as Runtime>::Word> + Into<<Self::Runtime as Runtime>::Word>
{
    fn read(&self) -> u64;
}

pub trait Error:
    ValueType + From<<Self::Runtime as Runtime>::Error> + Into<<Self::Runtime as Runtime>::Error>
{
    fn read(self) -> associated::Value<Self>;
}

pub trait Atom:
    ValueType + Eq + From<<Self::Runtime as Runtime>::Atom> + Into<<Self::Runtime as Runtime>::Atom>
{
}

pub trait Blob:
    ValueType + From<<Self::Runtime as Runtime>::Blob> + Into<<Self::Runtime as Runtime>::Blob>
{
    fn read(&self, buffer: &mut [u8]);

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Tree:
    ValueType + From<<Self::Runtime as Runtime>::Tree> + Into<<Self::Runtime as Runtime>::Tree>
{
    fn take(&mut self, index: usize) -> associated::Value<Self>;

    fn put(&mut self, index: usize, value: associated::Value<Self>) -> associated::Value<Self>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait Page:
    ValueType + From<<Self::Runtime as Runtime>::Page> + Into<<Self::Runtime as Runtime>::Page>
{
    fn read(&self, offset: usize, buffer: &mut [u8]);
    fn write(&mut self, offset: usize, buffer: &[u8]);

    fn size(&self) -> usize;
}

#[derive(Copy, Clone, Debug)]
pub struct MapError;

pub trait Table:
    ValueType + From<<Self::Runtime as Runtime>::Table> + Into<<Self::Runtime as Runtime>::Table>
where
    Entry<Self>: From<Entry<<Self::Runtime as Runtime>::Table>>,
    Entry<Self>: Into<Entry<<Self::Runtime as Runtime>::Table>>,
{
    fn take(&mut self, index: usize) -> Entry<Self>;
    fn put(&mut self, index: usize, entry: Entry<Self>) -> Result<Entry<Self>, Entry<Self>>;

    fn size(&self) -> usize;

    fn map(&mut self, address: usize, entry: Entry<Self>) -> Result<Entry<Self>, MapError> {
        let result = if address + entry.size() >= self.size() {
            try_replace_with(self, |this| {
                let rt = this.runtime();
                let mut embiggened = rt.create_table(this.size() * 512);
                embiggened
                    .put(0, Entry::RWTable(this.into()).into())
                    .map_err(|_| MapError)?;
                Ok(Self::from(embiggened))
            })?;
            self.map(address, entry)?
        } else if entry.size() == self.size() / 512 {
            let shift = entry.size().ilog2();
            let index = address >> shift;
            assert!(index < 512);
            self.put(index, entry).map_err(|_| MapError)?
        } else {
            let shift = (self.size() / 512).ilog2();
            let index = (address >> shift) & 0x1ff;
            let offset = address & !(0x1ff << shift);

            let mut smaller = match self.take(index) {
                Entry::ROTable(table) => table,
                Entry::RWTable(table) => table,
                _ => self.runtime().create_table(self.size() / 512),
            };
            assert!(self.size() > smaller.size());
            smaller.map(offset, entry.into()).map_err(|_| MapError)?;
            self.put(index, Entry::RWTable(smaller))
                .map_err(|_| MapError)?
        };
        Ok(result)
    }

    fn unmap(&mut self, address: usize) -> Option<Entry<Self>> {
        if address > self.size() {
            None
        } else {
            let shift = (self.size() / 512).ilog2();
            let index = (address >> shift) & 0x1ff;
            let offset = address & !(0x1ff << shift);

            let smaller = self.take(index);
            match smaller {
                Entry::ROTable(mut table) => {
                    let nested = table.unmap(offset);
                    assert!(self.put(index, Entry::ROTable(table)).is_ok());
                    nested.map(|x| x.into())
                }
                Entry::RWTable(mut table) => {
                    let nested = table.unmap(offset);
                    assert!(self.put(index, Entry::RWTable(table)).is_ok());
                    nested.map(|x| x.into())
                }
                x => Some(x),
            }
        }
    }
}

pub trait Lambda:
    ValueType + From<<Self::Runtime as Runtime>::Lambda> + Into<<Self::Runtime as Runtime>::Lambda>
{
    fn apply(self, argument: associated::Value<Self>) -> associated::Thunk<Self>;
    fn read(self) -> (associated::Thunk<Self>, usize);
}

pub trait Thunk:
    ValueType + From<<Self::Runtime as Runtime>::Thunk> + Into<<Self::Runtime as Runtime>::Thunk>
{
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
    + From<<Self::Runtime as Runtime>::Value>
    + Into<<Self::Runtime as Runtime>::Value>
{
    fn datatype(&self) -> DataType;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
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
pub enum Entry<T: Table>
where
    Entry<T>: From<Entry<<T::Runtime as Runtime>::Table>>,
    Entry<T>: Into<Entry<<T::Runtime as Runtime>::Table>>,
{
    Null(usize),
    ROPage(associated::Page<T>),
    RWPage(associated::Page<T>),
    ROTable(associated::Table<T>),
    RWTable(associated::Table<T>),
}

impl<T: Table> Entry<T>
where
    Entry<T>: From<Entry<<T::Runtime as Runtime>::Table>>,
    Entry<T>: Into<Entry<<T::Runtime as Runtime>::Table>>,
{
    pub fn size(&self) -> usize {
        match self {
            Entry::Null(size) => *size,
            Entry::ROPage(page) => page.size(),
            Entry::RWPage(page) => page.size(),
            Entry::ROTable(table) => table.size(),
            Entry::RWTable(table) => table.size(),
        }
    }
}

pub mod prelude {
    pub use super::{
        Atom as _, Blob as _, DataType, Error as _, Lambda as _, Null as _, Page as _,
        Runtime as _, RuntimeType as _, Table as _, Thunk as _, Tree as _, Value as _, ValueType,
        Word as _,
    };
}

// fn replace_with<T>(x: &mut T, f: impl FnOnce(T) -> T) {
//     unsafe {
//         let old = core::ptr::read(x);
//         let new = f(old);
//         core::ptr::write(x, new);
//     }
// }

fn try_replace_with<T, E>(x: &mut T, f: impl FnOnce(T) -> Result<T, E>) -> Result<(), E> {
    unsafe {
        let old = core::ptr::read(x);
        let new = f(old)?;
        core::ptr::write(x, new);
    }
    Ok(())
}
