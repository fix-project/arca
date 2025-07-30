#![no_std]
#![allow(unused)]

use core::{
    arch::{asm, global_asm},
    fmt::Write,
    marker::PhantomData,
    num::NonZero,
    sync::atomic::{AtomicUsize, Ordering},
};

pub mod error;
pub mod os;
pub mod prelude;
pub use arca;

extern crate defs;
use arca::Function;
use defs::SyscallError;
use defs::*;

use prelude::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Runtime;

static RUNTIME: Runtime = Runtime;

#[derive(Copy, Clone, Debug)]
pub enum ArcaError {
    BadSyscall,
    BadIndex,
    BadType,
    BadArgument,
    OutOfMemory,
    Interrupted,
    Unknown(u32),
}

fn syscall_result_raw(value: i64) -> Result<u32, ArcaError> {
    if value >= 0 {
        Ok(value as u32)
    } else {
        let err = -value as u32;
        Err(match err {
            defs::error::ERROR_BAD_SYSCALL => ArcaError::BadSyscall,
            defs::error::ERROR_BAD_INDEX => ArcaError::BadIndex,
            defs::error::ERROR_BAD_TYPE => ArcaError::BadType,
            defs::error::ERROR_BAD_ARGUMENT => ArcaError::BadArgument,
            defs::error::ERROR_OUT_OF_MEMORY => ArcaError::OutOfMemory,
            defs::error::ERROR_INTERRUPTED => ArcaError::Interrupted,
            x => ArcaError::Unknown(x),
        })
    }
}

fn syscall_result(value: i64) -> Result<Ref, ArcaError> {
    syscall_result_raw(value).map(Ref::from_raw)
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Ref {
    idx: Option<u32>,
}

impl Clone for Ref {
    fn clone(&self) -> Self {
        let idx = self.as_raw();
        unsafe { syscall_result(arca_clone(idx as i64)).unwrap() }
    }
}

impl Drop for Ref {
    fn drop(&mut self) {
        if let Some(idx) = self.idx.take() {
            unsafe {
                syscall_result_raw(arca_drop(idx as i64)).unwrap();
            }
        }
    }
}

impl Ref {
    fn from_raw(idx: u32) -> Self {
        Ref { idx: Some(idx) }
    }

    fn into_raw(mut self) -> u32 {
        self.idx.take().unwrap()
    }

    fn as_raw(&self) -> u32 {
        self.idx.unwrap()
    }
}

impl arca::Runtime for Runtime {
    type Null = Ref;
    type Word = Ref;
    type Exception = Ref;
    type Atom = Ref;
    type Blob = Ref;
    type Tuple = Ref;
    type Page = Ref;
    type Table = Ref;
    type Function = Ref;

    type Error = ArcaError;

    fn create_null() -> arca::Null<Self> {
        unsafe { arca::Null::from_inner(Ref::from_raw(0)) }
    }

    fn create_word(word: u64) -> arca::Word<Self> {
        unsafe { arca::Word::from_inner(syscall_result(defs::arca_word_create(word)).unwrap()) }
    }

    fn create_exception(value: arca::Value<Self>) -> arca::Exception<Self> {
        unsafe {
            arca::Exception::from_inner(
                syscall_result(defs::arca_exception_create(
                    Self::get_raw(value).into_raw().into(),
                ))
                .unwrap(),
            )
        }
    }

    fn create_atom(bytes: &[u8]) -> arca::Atom<Self> {
        unsafe {
            arca::Atom::from_inner(
                syscall_result(defs::arca_atom_create(bytes.as_ptr(), bytes.len())).unwrap(),
            )
        }
    }

    fn create_blob(bytes: &[u8]) -> arca::Blob<Self> {
        unsafe {
            arca::Blob::from_inner(
                syscall_result(defs::arca_blob_create(bytes.as_ptr(), bytes.len())).unwrap(),
            )
        }
    }

    fn create_tuple(len: usize) -> arca::Tuple<Self> {
        unsafe { arca::Tuple::from_inner(syscall_result(defs::arca_tuple_create(len)).unwrap()) }
    }

    fn create_page(len: usize) -> arca::Page<Self> {
        unsafe { arca::Page::from_inner(syscall_result(defs::arca_page_create(len)).unwrap()) }
    }

    fn create_table(len: usize) -> arca::Table<Self> {
        unsafe { arca::Table::from_inner(syscall_result(defs::arca_table_create(len)).unwrap()) }
    }

    fn create_function(
        arca: bool,
        data: arca::Value<Self>,
    ) -> Result<arca::Function<Self>, Self::Error> {
        unsafe {
            Ok(arca::Function::from_inner(syscall_result(
                defs::arca_function_create(arca, Self::get_raw(data).into_raw().into()),
            )?))
        }
    }

    fn value_len(value: arca::ValueRef<Self>) -> usize {
        unsafe {
            let idx: i64 = Self::get_raw_ref_idx(value).into();
            let mut length: usize = 0;
            let result = arca_length(idx, &mut length);
            syscall_result(result).unwrap();
            length
        }
    }

    fn read_word(word: &arca::Word<Self>) -> u64 {
        unsafe {
            let mut result = 0;
            syscall_result_raw(defs::arca_word_read(
                word.inner().as_raw() as i64,
                &mut result,
            ))
            .unwrap();
            result
        }
    }

    fn read_exception(exception: arca::Exception<Self>) -> arca::Value<Self> {
        unsafe {
            let data = syscall_result(defs::arca_exception_read(
                exception.into_inner().into_raw().into(),
            ))
            .unwrap();
            Self::raw_convert(data)
        }
    }

    fn read_blob(blob: &arca::Blob<Self>, offset: usize, buf: &mut [u8]) -> usize {
        unsafe {
            syscall_result_raw(defs::arca_blob_read(
                blob.inner().as_raw() as i64,
                offset,
                buf.as_mut_ptr(),
                buf.len(),
            ))
            .unwrap() as usize
        }
    }

    fn read_page(page: &arca::Page<Self>, offset: usize, buf: &mut [u8]) -> usize {
        unsafe {
            syscall_result_raw(defs::arca_page_read(
                page.inner().as_raw() as i64,
                offset,
                buf.as_mut_ptr(),
                buf.len(),
            ))
            .unwrap() as usize
        }
    }

    fn get_tuple(
        tuple: &arca::Tuple<Self>,
        index: usize,
    ) -> Result<arca::Value<Self>, Self::Error> {
        unsafe {
            Ok(Self::raw_convert(syscall_result(defs::arca_tuple_get(
                tuple.inner().as_raw() as i64,
                index,
            ))?))
        }
    }

    fn set_tuple(
        tuple: &mut arca::Tuple<Self>,
        index: usize,
        value: arca::Value<Self>,
    ) -> Result<arca::Value<Self>, Self::Error> {
        unsafe {
            Ok(Self::raw_convert(syscall_result(defs::arca_tuple_set(
                tuple.inner().as_raw() as i64,
                index,
                Self::get_raw(value).into_raw().into(),
            ))?))
        }
    }

    fn get_table(
        tuple: &arca::Table<Self>,
        index: usize,
    ) -> Result<arca::Entry<Self>, Self::Error> {
        todo!()
    }

    fn set_table(
        tuple: &mut arca::Table<Self>,
        index: usize,
        entry: arca::Entry<Self>,
    ) -> Result<arca::Entry<Self>, Self::Error> {
        todo!()
    }

    fn write_blob(blob: &mut arca::Blob<Self>, offset: usize, buf: &[u8]) -> usize {
        todo!()
    }

    fn write_page(page: &mut arca::Page<Self>, offset: usize, buf: &[u8]) -> usize {
        todo!()
    }

    fn apply_function(
        function: arca::Function<Self>,
        argument: arca::Value<Self>,
    ) -> arca::Function<Self> {
        unsafe {
            Function::from_inner(
                syscall_result(defs::arca_function_apply(
                    Self::get_raw(function.into()).into_raw().into(),
                    Self::get_raw(argument).into_raw().into(),
                ))
                .unwrap(),
            )
        }
    }

    fn force_function(function: arca::Function<Self>) -> arca::Value<Self> {
        unsafe {
            Self::raw_convert(
                syscall_result(defs::arca_function_force(
                    Self::get_raw(function.into()).into_raw().into(),
                ))
                .unwrap(),
            )
        }
    }

    fn is_function_arcane(function: &arca::Function<Self>) -> bool {
        todo!()
    }

    fn call_with_current_continuation(function: arca::Function<Self>) -> arca::Value<Self> {
        os::call_with_current_continuation(function)
    }
}

fn read_entry(entry: defs::entry) -> arca::Entry<Runtime> {
    unsafe {
        match entry {
            defs::entry {
                mode: defs::entry_mode::ENTRY_MODE_NONE,
                datatype: datatype::DATATYPE_NULL,
                data,
            } => arca::Entry::Null(data),
            defs::entry {
                mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
                datatype: datatype::DATATYPE_PAGE,
                data,
            } => arca::Entry::ROPage(arca::Page::from_inner(Ref::from_raw(data as u32))),
            defs::entry {
                mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
                datatype: datatype::DATATYPE_PAGE,
                data,
            } => arca::Entry::RWPage(arca::Page::from_inner(Ref::from_raw(data as u32))),
            defs::entry {
                mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
                datatype: datatype::DATATYPE_TABLE,
                data,
            } => arca::Entry::ROTable(arca::Table::from_inner(Ref::from_raw(data as u32))),
            defs::entry {
                mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
                datatype: datatype::DATATYPE_TABLE,
                data,
            } => arca::Entry::RWTable(arca::Table::from_inner(Ref::from_raw(data as u32))),
            _ => unreachable!(),
        }
    }
}

fn write_entry(entry: arca::Entry<Runtime>) -> defs::entry {
    match entry {
        arca::Entry::Null(size) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_NONE,
            datatype: datatype::DATATYPE_NULL,
            data: size,
        },
        arca::Entry::ROPage(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
            datatype: datatype::DATATYPE_PAGE,
            data: value.into_inner().into_raw() as usize,
        },
        arca::Entry::RWPage(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
            datatype: datatype::DATATYPE_PAGE,
            data: value.into_inner().into_raw() as usize,
        },
        arca::Entry::ROTable(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
            datatype: datatype::DATATYPE_TABLE,
            data: value.into_inner().into_raw() as usize,
        },
        arca::Entry::RWTable(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
            datatype: datatype::DATATYPE_TABLE,
            data: value.into_inner().into_raw() as usize,
        },
    }
}

impl Runtime {
    unsafe fn raw_datatype(data: Ref) -> arca::DataType {
        unsafe {
            match arca_type(data.as_raw() as i64) {
                defs::datatype::DATATYPE_NULL => arca::DataType::Null,
                defs::datatype::DATATYPE_WORD => arca::DataType::Word,
                defs::datatype::DATATYPE_ATOM => arca::DataType::Atom,
                defs::datatype::DATATYPE_EXCEPTION => arca::DataType::Exception,
                defs::datatype::DATATYPE_BLOB => arca::DataType::Blob,
                defs::datatype::DATATYPE_TUPLE => arca::DataType::Tuple,
                defs::datatype::DATATYPE_PAGE => arca::DataType::Page,
                defs::datatype::DATATYPE_TABLE => arca::DataType::Table,
                defs::datatype::DATATYPE_FUNCTION => arca::DataType::Function,
                _ => unreachable!(),
            }
        }
    }

    unsafe fn raw_convert(data: Ref) -> arca::Value<Self> {
        unsafe {
            match arca_type(data.as_raw() as i64) {
                defs::datatype::DATATYPE_NULL => Null::from_inner(data).into(),
                defs::datatype::DATATYPE_WORD => Word::from_inner(data).into(),
                defs::datatype::DATATYPE_ATOM => Atom::from_inner(data).into(),
                defs::datatype::DATATYPE_EXCEPTION => Exception::from_inner(data).into(),
                defs::datatype::DATATYPE_BLOB => Blob::from_inner(data).into(),
                defs::datatype::DATATYPE_TUPLE => Tuple::from_inner(data).into(),
                defs::datatype::DATATYPE_PAGE => Page::from_inner(data).into(),
                defs::datatype::DATATYPE_TABLE => Table::from_inner(data).into(),
                defs::datatype::DATATYPE_FUNCTION => Function::from_inner(data).into(),
                _ => unreachable!(),
            }
        }
    }

    unsafe fn get_raw(value: arca::Value<Self>) -> Ref {
        match value.into_inner() {
            arca::RawValue::Null(x) => x,
            arca::RawValue::Word(x) => x,
            arca::RawValue::Exception(x) => x,
            arca::RawValue::Atom(x) => x,
            arca::RawValue::Blob(x) => x,
            arca::RawValue::Tuple(x) => x,
            arca::RawValue::Page(x) => x,
            arca::RawValue::Table(x) => x,
            arca::RawValue::Function(x) => x,
        }
    }

    unsafe fn get_raw_ref_idx(value: arca::ValueRef<'_, Self>) -> u32 {
        match value.inner() {
            arca::RawValueRef::Null(x) => x,
            arca::RawValueRef::Word(x) => x,
            arca::RawValueRef::Exception(x) => x,
            arca::RawValueRef::Atom(x) => x,
            arca::RawValueRef::Blob(x) => x,
            arca::RawValueRef::Tuple(x) => x,
            arca::RawValueRef::Page(x) => x,
            arca::RawValueRef::Table(x) => x,
            arca::RawValueRef::Function(x) => x,
        }
        .idx
        .unwrap()
    }
}
