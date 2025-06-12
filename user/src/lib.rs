#![no_std]
#![allow(unused)]

use core::{
    arch::{asm, global_asm},
    fmt::Write,
    marker::PhantomData,
    sync::atomic::{AtomicUsize, Ordering},
};

pub use arca;
use arca::{DataType, Runtime as _, Value as _, ValueType, associated};
use defs::SyscallError;

extern crate defs;
use defs::*;

struct ErrorWriter;

impl ErrorWriter {
    pub fn reset(&self) {
        unsafe {
            arca_error_reset();
        }
    }

    pub fn exit(&self) {
        unsafe {
            arca_error_return();
        }
    }
}

impl core::fmt::Write for ErrorWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let result = unsafe { arca_error_append(s.as_ptr(), s.len()) };
        if result == 0 {
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

fn log(s: &str) {
    unsafe {
        arca_debug_log(s.as_ptr(), s.len());
    }
}

fn log_int(s: &str, x: u64) {
    unsafe {
        arca_debug_log_int(s.as_ptr(), s.len(), x);
    }
}

fn show(s: &str, x: i64) {
    unsafe {
        arca_debug_show(s.as_ptr(), s.len(), x);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    ErrorWriter.reset();
    let _ = writeln!(ErrorWriter, "{info}");
    ErrorWriter.exit();
    loop {
        unsafe {
            core::arch::asm!("ud2");
        }
        core::hint::spin_loop();
    }
}

pub struct Runtime;

static RUNTIME: Runtime = Runtime;

impl arca::Runtime for Runtime {
    type Null = Ref<Null>;
    type Word = Ref<Word>;
    type Error = Ref<Error>;
    type Atom = Ref<Atom>;
    type Blob = Ref<Blob>;
    type Tree = Ref<Tree>;
    type Page = Ref<Page>;
    type Table = Ref<Table>;
    type Lambda = Ref<Lambda>;
    type Thunk = Ref<Thunk>;
    type Value = Ref<Value>;

    fn create_null(&self) -> Self::Null {
        Ref::new(0)
    }

    fn create_word(&self, value: u64) -> Self::Word {
        let index = unsafe { arca_word_create(value) };
        Ref::try_new(index).unwrap()
    }

    fn create_error(&self, mut value: Self::Value) -> Self::Error {
        let index = value.index.take().unwrap();
        let index = unsafe { arca_error_create(index) };
        Ref::try_new(index).unwrap()
    }

    fn create_atom(&self, data: &[u8]) -> Self::Atom {
        let index = unsafe { arca_atom_create(data.as_ptr(), data.len()) };
        Ref::try_new(index).unwrap()
    }

    fn create_blob(&self, data: &[u8]) -> Self::Blob {
        let index = unsafe { arca_blob_create(data.as_ptr(), data.len()) };
        Ref::try_new(index).unwrap()
    }

    fn create_tree(&self, size: usize) -> Self::Tree {
        let index = unsafe { arca_tree_create(size) };
        Ref::try_new(index).unwrap()
    }

    fn create_page(&self, size: usize) -> Self::Page {
        let index = unsafe { arca_page_create(size) };
        Ref::try_new(index).unwrap()
    }

    fn create_table(&self, size: usize) -> Self::Table {
        let index = unsafe { arca_table_create(size) };
        Ref::try_new(index).unwrap()
    }

    fn create_lambda(&self, mut thunk: Self::Thunk, index: usize) -> Self::Lambda {
        let thunk = thunk.index.take().unwrap();
        let index = unsafe { arca_lambda_create(thunk, index) };
        Ref::try_new(index).unwrap()
    }

    fn create_thunk(
        &self,
        mut registers: Self::Blob,
        mut memory: Self::Table,
        mut descriptors: Self::Tree,
    ) -> Self::Thunk {
        let registers = registers.index.take().unwrap();
        let memory = memory.index.take().unwrap();
        let descriptors = descriptors.index.take().unwrap();
        let index = unsafe { arca_thunk_create(registers, memory, descriptors) };
        Ref::try_new(index).unwrap()
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Null;
#[derive(Copy, Clone, Debug)]
pub struct Word;
#[derive(Copy, Clone, Debug)]
pub struct Error;
#[derive(Copy, Clone, Debug)]
pub struct Atom;
#[derive(Copy, Clone, Debug)]
pub struct Blob;
#[derive(Copy, Clone, Debug)]
pub struct Tree;
#[derive(Copy, Clone, Debug)]
pub struct Page;
#[derive(Copy, Clone, Debug)]
pub struct Table;
#[derive(Copy, Clone, Debug)]
pub struct Lambda;
#[derive(Copy, Clone, Debug)]
pub struct Thunk;
#[derive(Copy, Clone, Debug)]
pub struct Value;

impl arca::ValueType for Ref<Null> {
    const DATATYPE: DataType = DataType::Null;
}
impl arca::ValueType for Ref<Word> {
    const DATATYPE: DataType = DataType::Word;
}
impl arca::ValueType for Ref<Error> {
    const DATATYPE: DataType = DataType::Error;
}
impl arca::ValueType for Ref<Atom> {
    const DATATYPE: DataType = DataType::Atom;
}
impl arca::ValueType for Ref<Blob> {
    const DATATYPE: DataType = DataType::Blob;
}
impl arca::ValueType for Ref<Tree> {
    const DATATYPE: DataType = DataType::Tree;
}
impl arca::ValueType for Ref<Page> {
    const DATATYPE: DataType = DataType::Page;
}
impl arca::ValueType for Ref<Table> {
    const DATATYPE: DataType = DataType::Table;
}
impl arca::ValueType for Ref<Lambda> {
    const DATATYPE: DataType = DataType::Lambda;
}
impl arca::ValueType for Ref<Thunk> {
    const DATATYPE: DataType = DataType::Thunk;
}

#[repr(transparent)]
#[derive(Debug)]
pub struct Ref<T> {
    index: Option<i64>,
    _phantom: PhantomData<T>,
}

impl<T> Ref<T> {
    fn new(index: i64) -> Self {
        Ref {
            index: Some(index),
            _phantom: PhantomData,
        }
    }

    fn try_new(index: i64) -> Result<Self, SyscallError> {
        if index >= 0 {
            Ok(Ref {
                index: Some(index),
                _phantom: PhantomData,
            })
        } else {
            Err(SyscallError::new(-index as u32))
        }
    }
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        let index = unsafe { arca_clone(self.index.unwrap()) };
        Ref::try_new(index).unwrap()
    }
}

impl<T> Drop for Ref<T> {
    fn drop(&mut self) {
        if let Some(index) = self.index {
            unsafe {
                assert_eq!(arca_drop(index), 0);
            }
        }
    }
}

impl<T> arca::RuntimeType for Ref<T> {
    type Runtime = Runtime;

    fn runtime(&self) -> &Self::Runtime {
        &Runtime
    }
}

impl arca::Null for Ref<Null> {}

impl arca::Word for Ref<Word> {
    fn read(&self) -> u64 {
        let mut x: u64 = 0;
        let p = &raw mut x;
        unsafe {
            assert_eq!(arca_word_read(self.index.unwrap(), p), 0);
        }
        x
    }
}

impl arca::Error for Ref<Error> {
    fn read(mut self) -> associated::Value<Self> {
        let idx = self.index.take().unwrap();
        let idx = unsafe { arca_error_read(idx) };
        Ref::try_new(idx).unwrap()
    }
}

impl arca::Atom for Ref<Atom> {}

impl PartialEq for Ref<Atom> {
    fn eq(&self, other: &Self) -> bool {
        let result = unsafe { arca_equals(self.index.unwrap(), other.index.unwrap()) };
        assert!(result >= 0);
        result != 0
    }
}

impl Eq for Ref<Atom> {}

impl arca::Blob for Ref<Blob> {
    fn read(&self, buffer: &mut [u8]) {
        unsafe {
            assert_eq!(
                arca_blob_read(self.index.unwrap(), buffer.as_mut_ptr(), buffer.len()),
                0
            );
        }
    }

    fn len(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(arca_length(self.index.unwrap(), &raw mut n), 0);
        }
        n
    }
}

impl arca::Tree for Ref<Tree> {
    fn take(&mut self, index: usize) -> associated::Value<Self> {
        let index = unsafe { arca_tree_take(self.index.unwrap(), index) };
        Ref::try_new(index).unwrap()
    }

    fn put(&mut self, index: usize, mut value: associated::Value<Self>) -> associated::Value<Self> {
        let index = unsafe { arca_tree_put(self.index.unwrap(), index, value.index.unwrap()) };
        Ref::try_new(index).unwrap()
    }

    fn len(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(arca_length(self.index.unwrap(), &raw mut n), 0);
        }
        n
    }
}

impl arca::Page for Ref<Page> {
    fn read(&self, offset: usize, buffer: &mut [u8]) {
        unsafe {
            assert_eq!(
                arca_page_read(
                    self.index.unwrap(),
                    offset,
                    buffer.as_mut_ptr(),
                    buffer.len()
                ),
                0
            );
        }
    }

    fn write(&mut self, offset: usize, buffer: &[u8]) {
        unsafe {
            assert_eq!(
                arca_page_write(self.index.unwrap(), offset, buffer.as_ptr(), buffer.len()),
                0
            );
        }
    }

    fn size(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(arca_length(self.index.unwrap(), &raw mut n), 0);
        }
        n
    }
}

impl arca::Table for Ref<Table> {
    fn take(&mut self, index: usize) -> arca::Entry<Self> {
        let mut entry = core::mem::MaybeUninit::uninit();
        let entry = unsafe {
            assert_eq!(
                arca_table_take(self.index.unwrap(), index, entry.as_mut_ptr()),
                0
            );
            entry.assume_init()
        };
        read_entry(entry)
    }

    fn put(
        &mut self,
        offset: usize,
        mut entry: arca::Entry<Self>,
    ) -> Result<arca::Entry<Self>, arca::Entry<Self>> {
        let mut entry = write_entry(entry);
        let result = unsafe { arca_table_put(self.index.unwrap(), offset, &mut entry) };
        let entry = read_entry(entry);
        if result == 0 { Ok(entry) } else { Err(entry) }
    }

    fn size(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(arca_length(self.index.unwrap(), &raw mut n), 0);
        }
        n
    }
}

pub type Entry = arca::Entry<Ref<Table>>;

impl arca::Lambda for Ref<Lambda> {
    fn apply(mut self, mut argument: associated::Value<Self>) -> associated::Thunk<Self> {
        let index = self.index.take().unwrap();
        let argument = argument.index.take().unwrap();
        let index = unsafe { arca_apply(index, argument) };
        Ref::try_new(index).unwrap()
    }

    fn read(self) -> (associated::Thunk<Self>, usize) {
        todo!()
    }
}

impl arca::Thunk for Ref<Thunk> {
    fn run(self) -> associated::Value<Self> {
        todo!()
    }

    fn read(
        self,
    ) -> (
        associated::Blob<Self>,
        associated::Table<Self>,
        associated::Tree<Self>,
    ) {
        todo!()
    }
}

impl arca::Value for Ref<Value> {
    fn datatype(&self) -> DataType {
        let typ: defs::datatype::Type = unsafe { arca_type(self.index.unwrap()) };
        match typ {
            defs::datatype::DATATYPE_NULL => DataType::Null,
            defs::datatype::DATATYPE_WORD => DataType::Word,
            defs::datatype::DATATYPE_ATOM => DataType::Atom,
            defs::datatype::DATATYPE_BLOB => DataType::Blob,
            defs::datatype::DATATYPE_TREE => DataType::Tree,
            defs::datatype::DATATYPE_PAGE => DataType::Page,
            defs::datatype::DATATYPE_TABLE => DataType::Table,
            defs::datatype::DATATYPE_LAMBDA => DataType::Lambda,
            defs::datatype::DATATYPE_THUNK => DataType::Thunk,
            _ => panic!("unrecognized type"),
        }
    }
}

impl<T> TryFrom<Ref<Value>> for Ref<T>
where
    Ref<T>: ValueType,
{
    type Error = Ref<Value>;

    fn try_from(mut value: Ref<Value>) -> Result<Self, Self::Error> {
        if value.datatype() == Ref::<T>::DATATYPE {
            Ok(Ref::new(value.index.take().unwrap()))
        } else {
            Err(value)
        }
    }
}

impl<T> From<Ref<T>> for Ref<Value>
where
    Ref<T>: ValueType,
{
    fn from(mut value: Ref<T>) -> Self {
        Ref::new(value.index.take().unwrap())
    }
}

impl Default for Ref<Value> {
    fn default() -> Self {
        os::null().into()
    }
}

impl<T: FnOnce() -> Ref<Value>> From<T> for Ref<Thunk> {
    fn from(value: T) -> Self {
        let mut continued: bool = false;
        let result = unsafe { arca_capture_continuation_thunk(&mut continued) };
        if continued {
            // in the resumed continuation
            let result = value();
            os::exit(result);
        };
        Ref::try_new(result).unwrap()
    }
}

impl<T: FnOnce(Ref<Value>) -> Ref<Value>> From<T> for Ref<Lambda> {
    fn from(value: T) -> Self {
        let mut continued: bool = false;
        let result = unsafe { arca_capture_continuation_lambda(&mut continued) };
        if continued {
            // in the resumed continuation
            let argument = Ref::try_new(result).unwrap();
            let result = value(argument);
            os::exit(result);
        };
        Ref::try_new(result).unwrap()
    }
}

pub mod os {
    pub use super::*;

    pub fn null() -> Ref<Null> {
        RUNTIME.create_null()
    }

    pub fn word(value: u64) -> Ref<Word> {
        RUNTIME.create_word(value)
    }

    pub fn atom(bytes: &[u8]) -> Ref<Atom> {
        RUNTIME.create_atom(bytes)
    }

    pub fn blob(data: &[u8]) -> Ref<Blob> {
        RUNTIME.create_blob(data)
    }

    pub fn tree(size: usize) -> Ref<Tree> {
        RUNTIME.create_tree(size)
    }

    pub fn log(s: &str) {
        super::log(s);
    }

    pub fn show(s: &str, x: &Ref<Value>) {
        super::show(s, x.index.unwrap());
    }

    pub fn prompt() -> Ref<Value> {
        let result = unsafe { arca_return_continuation_lambda() };
        Ref::try_new(result).unwrap()
    }

    pub fn perform<T: Into<Ref<Value>>>(effect: T) -> Ref<Value> {
        let mut val: Ref<Value> = effect.into();
        let idx = val.index.take().unwrap();
        let idx = unsafe { arca_perform_effect(idx) };
        Ref::try_new(idx).unwrap()
    }

    pub fn exit<T: Into<Ref<Value>>>(value: T) -> ! {
        let mut val: Ref<Value> = value.into();
        let idx = val.index.take().unwrap();
        unsafe {
            arca_exit(idx);
            asm!("ud2");
        }
        unreachable!();
    }

    pub fn tailcall(mut value: Ref<Thunk>) -> ! {
        unsafe {
            arca_tailcall(value.index.take().unwrap());
            asm!("ud2");
        }
        unreachable!();
    }
}

pub mod prelude {
    pub use super::*;
    pub use arca::{
        Atom as _, Blob as _, DataType, Error as _, Lambda as _, Null as _, Page as _, Table as _,
        Thunk as _, Tree as _, Value as _, Word as _,
    };
}

fn read_entry(entry: defs::entry) -> Entry {
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
        } => arca::Entry::ROPage(Ref::new(data as i64)),
        defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
            datatype: datatype::DATATYPE_PAGE,
            data,
        } => arca::Entry::RWPage(Ref::new(data as i64)),
        defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
            datatype: datatype::DATATYPE_TABLE,
            data,
        } => arca::Entry::ROTable(Ref::new(data as i64)),
        defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
            datatype: datatype::DATATYPE_TABLE,
            data,
        } => arca::Entry::RWTable(Ref::new(data as i64)),
        _ => unreachable!(),
    }
}

fn write_entry(entry: Entry) -> defs::entry {
    match entry {
        arca::Entry::Null(size) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_NONE,
            datatype: datatype::DATATYPE_NULL,
            data: size,
        },
        arca::Entry::ROPage(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
            datatype: datatype::DATATYPE_PAGE,
            data: value.index.take().unwrap() as usize,
        },
        arca::Entry::RWPage(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
            datatype: datatype::DATATYPE_PAGE,
            data: value.index.take().unwrap() as usize,
        },
        arca::Entry::ROTable(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_ONLY,
            datatype: datatype::DATATYPE_TABLE,
            data: value.index.take().unwrap() as usize,
        },
        arca::Entry::RWTable(mut value) => defs::entry {
            mode: defs::entry_mode::ENTRY_MODE_READ_WRITE,
            datatype: datatype::DATATYPE_TABLE,
            data: value.index.take().unwrap() as usize,
        },
    }
}
