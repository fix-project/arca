use super::internal;
use crate::{prelude::*, types::arca::Arca};

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Runtime;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    InvalidTableEntry(super::Entry),
    InvalidIndex(usize),
    InvalidValue(arca::Value<Runtime>),
}

impl arca::Runtime for Runtime {
    type Null = super::internal::Null;
    type Word = super::internal::Word;
    type Exception = super::internal::Exception;
    type Atom = super::internal::Atom;
    type Blob = super::internal::Blob;
    type Tuple = super::internal::Tuple;
    type Page = super::internal::Page;
    type Table = super::internal::Table;
    type Function = super::internal::Function;

    type Error = Error;

    fn create_null() -> arca::Null<Self> {
        arca::Null::from_inner(internal::Null::new())
    }

    fn create_word(word: u64) -> arca::Word<Self> {
        arca::Word::from_inner(internal::Word::new(word))
    }

    fn create_exception(value: arca::Value<Self>) -> arca::Exception<Self> {
        arca::Exception::from_inner(internal::Exception::new(value))
    }

    fn create_atom(bytes: &[u8]) -> arca::Atom<Self> {
        arca::Atom::from_inner(internal::Atom::new(bytes))
    }

    fn create_blob(bytes: &[u8]) -> arca::Blob<Self> {
        arca::Blob::from_inner(internal::Blob::new(bytes))
    }

    fn create_tuple(len: usize) -> arca::Tuple<Self> {
        arca::Tuple::from_inner(internal::Tuple::new_with_len(len))
    }

    fn create_page(len: usize) -> arca::Page<Self> {
        arca::Page::from_inner(internal::Page::new(len))
    }

    fn create_table(len: usize) -> arca::Table<Self> {
        arca::Table::from_inner(internal::Table::new(len))
    }

    fn create_function(
        arca: bool,
        data: arca::Value<Self>,
    ) -> Result<arca::Function<Self>, Self::Error> {
        Ok(arca::Function::from_inner(if arca {
            let data: arca::Tuple<Self> = data.try_into().map_err(Error::InvalidValue)?;
            let registers: arca::Blob<Self> =
                data.get(0).try_into().map_err(Error::InvalidValue)?;
            let memory: arca::Table<Self> = data.get(1).try_into().map_err(Error::InvalidValue)?;
            let descriptors: arca::Tuple<Self> =
                data.get(2).try_into().map_err(Error::InvalidValue)?;

            let registers = registers.into_inner();
            let registers: Vec<u64> = registers
                .chunks(8)
                .map(|x| u64::from_ne_bytes(x.try_into().unwrap()))
                .collect();
            let mut register_file = RegisterFile::new();
            for (i, x) in registers.iter().take(18).enumerate() {
                register_file[i] = *x;
            }
            let memory = memory.into_inner();
            let descriptors = descriptors.into_inner();
            let arca = Arca::new_with(register_file, memory, descriptors);
            internal::Function::arcane(arca)
        } else {
            internal::Function::symbolic(data)
        }))
    }

    fn value_len(value: arca::ValueRef<Self>) -> usize {
        match value.inner() {
            arca::RawValueRef::Null(_) => 0,
            arca::RawValueRef::Word(x) => core::mem::size_of_val(x),
            arca::RawValueRef::Exception(_) => todo!(),
            arca::RawValueRef::Atom(_) => todo!(),
            arca::RawValueRef::Blob(x) => x.len(),
            arca::RawValueRef::Tuple(x) => x.len(),
            arca::RawValueRef::Page(x) => x.size(),
            arca::RawValueRef::Table(x) => x.size(),
            arca::RawValueRef::Function(_) => todo!(),
        }
    }

    fn read_word(word: &arca::Word<Self>) -> u64 {
        word.inner().read()
    }

    fn read_exception(exception: arca::Exception<Self>) -> arca::Value<Self> {
        exception.into_inner().read()
    }

    fn read_blob(blob: &arca::Blob<Self>, offset: usize, buf: &mut [u8]) -> usize {
        let len = core::cmp::min(buf.len(), blob.len() - offset);
        buf[..len].copy_from_slice(&blob[offset..offset + len]);
        len
    }

    fn read_page(page: &arca::Page<Self>, offset: usize, buf: &mut [u8]) -> usize {
        let len = core::cmp::min(buf.len(), page.len() - offset);
        buf[..len].copy_from_slice(&page[offset..offset + len]);
        len
    }

    fn write_blob(blob: &mut arca::Blob<Self>, offset: usize, buf: &[u8]) -> usize {
        let len = core::cmp::min(buf.len(), blob.len() - offset);
        let end = offset + len;
        blob[offset..end].copy_from_slice(buf);
        len
    }

    fn write_page(page: &mut arca::Page<Self>, offset: usize, buf: &[u8]) -> usize {
        let len = core::cmp::min(buf.len(), page.len() - offset);
        let end = offset + len;
        page[offset..end].copy_from_slice(buf);
        len
    }

    fn get_tuple(
        tuple: &arca::Tuple<Self>,
        index: usize,
    ) -> Result<arca::Value<Self>, Self::Error> {
        let inner = tuple.inner();
        if index >= inner.len() {
            return Err(Error::InvalidIndex(index));
        }
        Ok(inner[index].clone())
    }

    fn set_tuple(
        tuple: &mut arca::Tuple<Self>,
        index: usize,
        value: arca::Value<Self>,
    ) -> Result<arca::Value<Self>, Self::Error> {
        let inner = tuple.inner_mut();
        if index >= inner.len() {
            return Err(Error::InvalidIndex(index));
        }
        Ok(core::mem::replace(&mut inner[index], value))
    }

    fn get_table(
        table: &arca::Table<Self>,
        index: usize,
    ) -> Result<arca::Entry<Self>, Self::Error> {
        let inner = table.inner();
        Ok(inner.get(index))
    }

    fn set_table(
        table: &mut arca::Table<Self>,
        index: usize,
        entry: arca::Entry<Self>,
    ) -> Result<arca::Entry<Self>, Self::Error> {
        let inner = table.inner_mut();
        let old = inner.set(index, entry).map_err(Error::InvalidTableEntry)?;
        Ok(old)
    }

    fn apply_function(
        mut function: arca::Function<Self>,
        argument: arca::Value<Self>,
    ) -> arca::Function<Self> {
        let inner = function.inner_mut();
        inner.apply(argument);
        function
    }

    fn force_function(function: arca::Function<Self>) -> arca::Value<Self> {
        let inner = function.into_inner();
        inner.force()
    }

    fn is_function_arcane(function: &arca::Function<Self>) -> bool {
        function.inner().is_arcane()
    }

    fn call_with_current_continuation(_: arca::Function<Self>) -> arca::Value<Self> {
        panic!("call/cc is not supported on the in-kernel runtime!");
    }
}
