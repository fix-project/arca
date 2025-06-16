use crate::prelude::*;

pub struct Runtime;

impl arca::Runtime for Runtime {
    type Null = Null;
    type Word = Word;
    type Error = Error;
    type Atom = Atom;
    type Blob = Blob;
    type Tree = Tree;
    type Page = Page;
    type Table = Table;
    type Lambda = Lambda;
    type Thunk = Thunk;
    type Value = Value;

    fn create_null(&self) -> Self::Null {
        Null
    }

    fn create_word(&self, value: u64) -> Self::Word {
        Word::new(value)
    }

    fn create_error(&self, value: Self::Value) -> Self::Error {
        Error::new(value)
    }

    fn create_atom(&self, data: &[u8]) -> Self::Atom {
        Atom::new(data)
    }

    fn create_blob(&self, data: &[u8]) -> Self::Blob {
        Blob::new(data)
    }

    fn create_tree(&self, size: usize) -> Self::Tree {
        Tree::new_with_len(size)
    }

    fn create_page(&self, size: usize) -> Self::Page {
        Page::new(size)
    }

    fn create_table(&self, size: usize) -> Self::Table {
        Table::new(size)
    }

    fn create_lambda(&self, _thunk: Self::Thunk, _index: usize) -> Self::Lambda {
        todo!()
    }

    fn create_thunk(
        &self,
        registers: Self::Blob,
        memory: Self::Table,
        descriptors: Self::Tree,
    ) -> Self::Thunk {
        let registers: Vec<u64> = registers
            .chunks(8)
            .map(|x| u64::from_ne_bytes(x.try_into().unwrap()))
            .collect();
        let mut register_file = RegisterFile::new();
        for (i, x) in registers.iter().take(18).enumerate() {
            register_file[i] = *x;
        }
        let arca = Arca::new_with(register_file, memory, descriptors);
        Thunk::new(arca)
    }
}
