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
pub mod buffer;
pub mod io;

use arca::Function;
use arcane::SyscallError;
use arcane::*;

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
    Interrupted,
    Unknown(u32),
}

fn syscall_result_raw(value: i64) -> Result<u32, ArcaError> {
    if value >= 0 {
        Ok(value as u32)
    } else {
        let err = -value as u32;
        Err(match err {
            arcane::__ERR_bad_syscall => ArcaError::BadSyscall,
            arcane::__ERR_bad_index => ArcaError::BadIndex,
            arcane::__ERR_bad_type => ArcaError::BadType,
            arcane::__ERR_bad_argument => ArcaError::BadArgument,
            arcane::__ERR_interrupted => ArcaError::Interrupted,
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
        unsafe { arca::Word::from_inner(syscall_result(arcane::arca_word_create(word)).unwrap()) }
    }

    fn create_blob(bytes: &[u8]) -> arca::Blob<Self> {
        unsafe {
            arca::Blob::from_inner(
                syscall_result(arcane::arca_blob_create(bytes.as_ptr(), bytes.len())).unwrap(),
            )
        }
    }

    fn create_tuple(len: usize) -> arca::Tuple<Self> {
        unsafe { arca::Tuple::from_inner(syscall_result(arcane::arca_tuple_create(len)).unwrap()) }
    }

    fn create_page(len: usize) -> arca::Page<Self> {
        unsafe { arca::Page::from_inner(syscall_result(arcane::arca_page_create(len)).unwrap()) }
    }

    fn create_table(len: usize) -> arca::Table<Self> {
        unsafe { arca::Table::from_inner(syscall_result(arcane::arca_table_create(len)).unwrap()) }
    }

    fn create_function(data: arca::Value<Self>) -> Result<arca::Function<Self>, Self::Error> {
        unsafe {
            Ok(arca::Function::from_inner(syscall_result(
                arcane::arca_function_create(Self::get_raw(data).into_raw().into()),
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
            syscall_result_raw(arcane::arca_word_read(
                word.inner().as_raw() as i64,
                &mut result,
            ))
            .unwrap();
            result
        }
    }

    fn read_blob(blob: &arca::Blob<Self>, offset: usize, buf: &mut [u8]) -> usize {
        unsafe {
            syscall_result_raw(arcane::arca_blob_read(
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
            syscall_result_raw(arcane::arca_page_read(
                page.inner().as_raw() as i64,
                offset,
                buf.as_mut_ptr(),
                buf.len(),
            ))
            .unwrap() as usize
        }
    }

    fn read_function(function: Function<Self>) -> arca::Value<Self> {
        unsafe {
            Self::raw_convert(
                syscall_result(arcane::arca_function_read(function.inner().as_raw() as i64))
                    .unwrap(),
            )
        }
    }

    fn get_tuple(
        tuple: &arca::Tuple<Self>,
        index: usize,
    ) -> Result<arca::Value<Self>, Self::Error> {
        unsafe {
            Ok(Self::raw_convert(syscall_result(arcane::arca_tuple_get(
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
            Ok(Self::raw_convert(syscall_result(arcane::arca_tuple_set(
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
        unsafe {
            syscall_result_raw(arcane::arca_blob_write(
                blob.inner().as_raw() as i64,
                offset,
                buf.as_ptr(),
                buf.len(),
            ))
            .unwrap() as usize
        }
    }

    fn write_page(page: &mut arca::Page<Self>, offset: usize, buf: &[u8]) -> usize {
        unsafe {
            syscall_result_raw(arcane::arca_page_write(
                page.inner().as_raw() as i64,
                offset,
                buf.as_ptr(),
                buf.len(),
            ))
            .unwrap() as usize
        }
    }

    fn apply_function(
        function: arca::Function<Self>,
        argument: arca::Value<Self>,
    ) -> arca::Function<Self> {
        unsafe {
            Function::from_inner(
                syscall_result(arcane::arca_function_apply(
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
                syscall_result(arcane::arca_function_force(
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

fn read_entry(entry: arcane::arca_entry) -> arca::Entry<Runtime> {
    unsafe {
        match entry {
            arcane::arca_entry {
                mode: arcane::__MODE_none,
                datatype: arcane::__TYPE_null,
                data,
            } => arca::Entry::Null(data),
            arcane::arca_entry {
                mode: arcane::__MODE_read_only,
                datatype: arcane::__TYPE_page,
                data,
            } => arca::Entry::ROPage(arca::Page::from_inner(Ref::from_raw(data as u32))),
            arcane::arca_entry {
                mode: arcane::__MODE_read_write,
                datatype: arcane::__TYPE_page,
                data,
            } => arca::Entry::RWPage(arca::Page::from_inner(Ref::from_raw(data as u32))),
            arcane::arca_entry {
                mode: arcane::__MODE_read_only,
                datatype: arcane::__TYPE_table,
                data,
            } => arca::Entry::ROTable(arca::Table::from_inner(Ref::from_raw(data as u32))),
            arcane::arca_entry {
                mode: arcane::__MODE_read_write,
                datatype: arcane::__TYPE_table,
                data,
            } => arca::Entry::RWTable(arca::Table::from_inner(Ref::from_raw(data as u32))),
            _ => unreachable!(),
        }
    }
}

fn write_entry(entry: arca::Entry<Runtime>) -> arcane::arca_entry {
    match entry {
        arca::Entry::Null(size) => arcane::arca_entry {
            mode: arcane::__MODE_none,
            datatype: arcane::__TYPE_null,
            data: size,
        },
        arca::Entry::ROPage(mut value) => arcane::arca_entry {
            mode: arcane::__MODE_read_only,
            datatype: arcane::__TYPE_page,
            data: value.into_inner().into_raw() as usize,
        },
        arca::Entry::RWPage(mut value) => arcane::arca_entry {
            mode: arcane::__MODE_read_write,
            datatype: arcane::__TYPE_page,
            data: value.into_inner().into_raw() as usize,
        },
        arca::Entry::ROTable(mut value) => arcane::arca_entry {
            mode: arcane::__MODE_read_only,
            datatype: arcane::__TYPE_table,
            data: value.into_inner().into_raw() as usize,
        },
        arca::Entry::RWTable(mut value) => arcane::arca_entry {
            mode: arcane::__MODE_read_write,
            datatype: arcane::__TYPE_table,
            data: value.into_inner().into_raw() as usize,
        },
    }
}

impl Runtime {
    unsafe fn raw_datatype(data: Ref) -> arca::DataType {
        unsafe {
            match arca_type(data.as_raw() as i64) as u32 {
                arcane::__TYPE_null => arca::DataType::Null,
                arcane::__TYPE_word => arca::DataType::Word,
                arcane::__TYPE_blob => arca::DataType::Blob,
                arcane::__TYPE_tuple => arca::DataType::Tuple,
                arcane::__TYPE_page => arca::DataType::Page,
                arcane::__TYPE_table => arca::DataType::Table,
                arcane::__TYPE_function => arca::DataType::Function,
                _ => unreachable!(),
            }
        }
    }

    unsafe fn raw_convert(data: Ref) -> arca::Value<Self> {
        unsafe {
            match arca_type(data.as_raw() as i64) as u32 {
                arcane::__TYPE_null => Null::from_inner(data).into(),
                arcane::__TYPE_word => Word::from_inner(data).into(),
                arcane::__TYPE_blob => Blob::from_inner(data).into(),
                arcane::__TYPE_tuple => Tuple::from_inner(data).into(),
                arcane::__TYPE_page => Page::from_inner(data).into(),
                arcane::__TYPE_table => Table::from_inner(data).into(),
                arcane::__TYPE_function => Function::from_inner(data).into(),
                _ => unreachable!(),
            }
        }
    }

    unsafe fn get_raw(value: arca::Value<Self>) -> Ref {
        match value.into_inner() {
            arca::RawValue::Null(x) => x,
            arca::RawValue::Word(x) => x,
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

#[cfg(feature = "allocator")]
mod allocator {
    use core::ffi::c_void;

    use arca::Entry;
    use arcane::{__MODE_read_write, arca_compat_mmap, arca_mmap};
    use spin::{Mutex, lazy::Lazy};
    use talc::{ClaimOnOom, OomHandler, Span, Talc, Talck};

    use crate::{prelude::Page, write_entry};

    unsafe extern "C" {
        static __stack_top: c_void;
    }
    static HEAP: Lazy<Mutex<usize>> = Lazy::new(|| Mutex::new(&raw const __stack_top as usize));

    #[global_allocator]
    static ALLOCATOR: Talck<spin::Mutex<()>, Mmap> = Talc::new(Mmap).lock();

    struct Mmap;

    impl OomHandler for Mmap {
        fn handle_oom(talc: &mut Talc<Self>, layout: core::alloc::Layout) -> Result<(), ()> {
            // TODO(kmohr) review this
            const PAGE_SIZE: usize = 4096;

            // Calculate the total size needed, considering alignment requirements
            // TODO(kmohr) I'm just arbitrarily adding 128 bytes
            // what is the exact amount of space needed for metadata?
            let align = layout.align().max(PAGE_SIZE);
            let required_size = (layout.size() + 128).max(PAGE_SIZE);

            // Round up to alignment boundary
            let aligned_size = (required_size + align - 1) & !(align - 1);
            let pages_needed = aligned_size.div_ceil(PAGE_SIZE);
            let mut total_size = pages_needed * PAGE_SIZE;

            let mut addr = HEAP.lock();
            let current_addr = *addr;

            // Align the base address to the required alignment
            let aligned_base = (current_addr + align - 1) & !(align - 1);

            let page_addr = aligned_base;

            unsafe {
                let base = page_addr as *mut c_void;
                let len= arca_compat_mmap(base, total_size, __MODE_read_write);
                if len < 0 {
                    panic!("Failed to handle oom");
                }
                total_size = len as usize;
            }

            // Claim the entire aligned region for the allocator
            unsafe {
                let base = aligned_base as *mut u8;
                let end = base.add(total_size);
                talc.claim(Span::new(base, end));
            }

            // Update heap pointer to after the allocated region
            *addr = aligned_base + total_size;
            Ok(())
        }
    }
}
