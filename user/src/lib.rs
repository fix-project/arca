#![no_std]
#![allow(unused)]

use core::{
    arch::{asm, global_asm},
    sync::atomic::{AtomicUsize, Ordering},
};

extern crate defs;

global_asm!(
    "
.globl syscall
syscall:
    mov r10, rcx
    syscall
    ret
"
);

unsafe extern "C" {
    pub fn syscall(num: u32, ...) -> i64;
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("ud2");
        }
        core::hint::spin_loop();
    }
}

static NEXT_DESCRIPTOR: AtomicUsize = AtomicUsize::new(0);
static COUNT_DESCRIPTOR: AtomicUsize = AtomicUsize::new(0);

fn next_descriptor() -> usize {
    let mut next = NEXT_DESCRIPTOR.load(Ordering::SeqCst);
    let mut count = COUNT_DESCRIPTOR.load(Ordering::SeqCst);

    if next == count {
        if count == 0 {
            count = 16;
        } else {
            count *= 2;
        }

        unsafe {
            let result = syscall(defs::syscall::SYS_RESIZE, count);
            assert_eq!(result, 0);
        }
        COUNT_DESCRIPTOR.store(count, Ordering::SeqCst);
    }
    NEXT_DESCRIPTOR.fetch_add(1, Ordering::SeqCst)
}

use core::marker::PhantomData;

pub use arca;
use arca::{Runtime as _, associated};

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
    type AnyValue = Ref<Value>;

    fn create_null(&self) -> Self::Null {
        let index = next_descriptor();
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_CREATE_NULL, index), 0);
        }
        Ref::new(index)
    }

    fn create_word(&self, value: u64) -> Self::Word {
        let index = next_descriptor();
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_CREATE_WORD, index, value), 0);
        }
        Ref::new(index)
    }

    fn create_error(&self, value: Self::AnyValue) -> Self::Error {
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_CREATE_ERROR, value.index, value.index),
                0
            );
        }
        Ref::new(value.index)
    }

    fn create_atom(&self, data: &[u8]) -> Self::Atom {
        let index = next_descriptor();
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_CREATE_ATOM,
                    index,
                    data.as_ptr(),
                    data.len()
                ),
                0
            );
        }
        Ref::new(index)
    }

    fn create_blob(&self, data: &[u8]) -> Self::Blob {
        let index = next_descriptor();
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_CREATE_BLOB,
                    index,
                    data.as_ptr(),
                    data.len()
                ),
                0
            );
        }
        Ref::new(index)
    }

    fn create_tree(&self, values: &mut [Self::AnyValue]) -> Self::Tree {
        let index = next_descriptor();
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_CREATE_TREE,
                    index,
                    values.as_ptr(),
                    values.len()
                ),
                0
            );
        }
        Ref::new(index)
    }

    fn create_page(&self, size: usize) -> Self::Page {
        let index = next_descriptor();
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_CREATE_PAGE, index, size), 0);
        }
        Ref::new(index)
    }

    fn create_table(&self, size: usize) -> Self::Table {
        let index = next_descriptor();
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_CREATE_TABLE, index, size), 0);
        }
        Ref::new(index)
    }

    fn create_lambda(&self, thunk: Self::Thunk, index: usize) -> Self::Lambda {
        let thunk = thunk.index;
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_CREATE_LAMBDA, thunk, thunk, index),
                0
            );
        }
        Ref::new(thunk)
    }

    fn create_thunk(
        &self,
        registers: Self::Blob,
        memory: Self::Table,
        descriptors: Self::Tree,
    ) -> Self::Thunk {
        let registers = registers.index;
        let memory = memory.index;
        let descriptors = descriptors.index;
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_CREATE_THUNK,
                    registers,
                    registers,
                    memory,
                    descriptors
                ),
                0
            );
        }
        Ref::new(registers)
    }
}

#[derive(Copy, Clone)]
pub struct Null;
#[derive(Copy, Clone)]
pub struct Word;
#[derive(Copy, Clone)]
pub struct Error;
#[derive(Copy, Clone)]
pub struct Atom;
#[derive(Copy, Clone)]
pub struct Blob;
#[derive(Copy, Clone)]
pub struct Tree;
#[derive(Copy, Clone)]
pub struct Page;
#[derive(Copy, Clone)]
pub struct Table;
#[derive(Copy, Clone)]
pub struct Lambda;
#[derive(Copy, Clone)]
pub struct Thunk;
#[derive(Copy, Clone)]
pub struct Value;

pub trait ValueType {}
impl ValueType for Null {}
impl ValueType for Word {}
impl ValueType for Error {}
impl ValueType for Atom {}
impl ValueType for Blob {}
impl ValueType for Tree {}
impl ValueType for Page {}
impl ValueType for Table {}
impl ValueType for Lambda {}
impl ValueType for Thunk {}

#[repr(transparent)]
pub struct Ref<T> {
    index: usize,
    _phantom: PhantomData<T>,
}

impl<T> Ref<T> {
    fn new(index: usize) -> Self {
        Ref {
            index,
            _phantom: PhantomData,
        }
    }
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        let old = self.index;
        let new = next_descriptor();
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_CLONE, new, old), 0);
        }
        Ref::new(new)
    }
}

impl<T> arca::Value for Ref<T> {
    type Runtime = Runtime;
}

impl<T> From<Ref<T>> for arca::DynValue<Runtime> {
    fn from(value: Ref<T>) -> Self {
        let index = value.index;
        let typ: defs::datatype::Type = unsafe {
            let result = syscall(defs::syscall::SYS_TYPE, index);
            result
                .try_into()
                .expect("got error trying to get value's datatype")
        };
        match typ {
            defs::datatype::DATATYPE_NULL => arca::DynValue::Null(Ref::new(index)),
            defs::datatype::DATATYPE_WORD => arca::DynValue::Word(Ref::new(index)),
            defs::datatype::DATATYPE_ATOM => arca::DynValue::Atom(Ref::new(index)),
            defs::datatype::DATATYPE_BLOB => arca::DynValue::Blob(Ref::new(index)),
            defs::datatype::DATATYPE_TREE => arca::DynValue::Tree(Ref::new(index)),
            defs::datatype::DATATYPE_PAGE => arca::DynValue::Page(Ref::new(index)),
            defs::datatype::DATATYPE_TABLE => arca::DynValue::Table(Ref::new(index)),
            defs::datatype::DATATYPE_LAMBDA => arca::DynValue::Lambda(Ref::new(index)),
            defs::datatype::DATATYPE_THUNK => arca::DynValue::Thunk(Ref::new(index)),
            _ => panic!("unrecognized type"),
        }
    }
}

impl From<arca::DynValue<Runtime>> for Ref<Value> {
    fn from(value: arca::DynValue<Runtime>) -> Self {
        let index = match value {
            arca::DynValue::Null(Ref { index, .. }) => index,
            arca::DynValue::Word(Ref { index, .. }) => index,
            arca::DynValue::Error(Ref { index, .. }) => index,
            arca::DynValue::Atom(Ref { index, .. }) => index,
            arca::DynValue::Blob(Ref { index, .. }) => index,
            arca::DynValue::Tree(Ref { index, .. }) => index,
            arca::DynValue::Page(Ref { index, .. }) => index,
            arca::DynValue::Table(Ref { index, .. }) => index,
            arca::DynValue::Lambda(Ref { index, .. }) => index,
            arca::DynValue::Thunk(Ref { index, .. }) => index,
        };
        Ref::new(index)
    }
}

impl arca::Null for Ref<Null> {}

impl arca::Word for Ref<Word> {
    fn read(&self) -> u64 {
        let mut x: u64 = 0;
        let p = &raw mut x;
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_READ, self.index, p), 0);
        }
        x
    }
}

impl arca::Error for Ref<Error> {
    fn read(self) -> associated::AnyValue<Self> {
        let idx = self.index;
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_READ, idx, idx), 0);
        }
        Ref::new(idx)
    }
}

impl arca::Atom for Ref<Atom> {}

impl PartialEq for Ref<Atom> {
    fn eq(&self, other: &Self) -> bool {
        let result = unsafe { syscall(defs::syscall::SYS_EQUALS, self.index, other.index) };
        assert!(result >= 0);
        result != 0
    }
}

impl Eq for Ref<Atom> {}

impl arca::Blob for Ref<Blob> {
    fn read(&self, buffer: &mut [u8]) {
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_READ,
                    self.index,
                    buffer.as_ptr(),
                    buffer.len()
                ),
                0
            );
        }
    }

    fn len(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_LENGTH, self.index, &raw mut n as u64),
                0
            );
        }
        n
    }
}

impl arca::Tree for Ref<Tree> {
    fn read(&self, buffer: &mut [associated::AnyValue<Self>]) {
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_READ,
                    self.index,
                    buffer.as_ptr(),
                    buffer.len()
                ),
                0
            );
        }
    }

    fn take(&mut self, index: usize) -> associated::AnyValue<Self> {
        let new = next_descriptor();
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_TAKE, new, self.index, index), 0);
        }
        Ref::new(new)
    }

    fn put(
        &mut self,
        index: usize,
        value: associated::AnyValue<Self>,
    ) -> associated::AnyValue<Self> {
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_PUT,
                    value.index,
                    self.index,
                    value.index,
                    index
                ),
                0
            );
        }
        Ref::new(value.index)
    }

    fn len(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_LENGTH, self.index, &raw mut n as u64),
                0
            );
        }
        n
    }
}

impl arca::Page for Ref<Page> {
    fn read(&self, offset: usize, buffer: &mut [u8]) {
        todo!()
    }

    fn write(&mut self, offset: usize, buffer: &[u8]) {
        todo!()
    }

    fn size(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_LENGTH, self.index, &raw mut n as u64),
                0
            );
        }
        n
    }
}

impl arca::Table for Ref<Table> {
    fn take(&mut self, index: usize) -> arca::Entry<Self> {
        todo!()
    }

    fn put(
        &mut self,
        offset: usize,
        entry: arca::Entry<Self>,
    ) -> Result<arca::Entry<Self>, arca::Entry<Self>> {
        todo!()
    }

    fn size(&self) -> usize {
        let mut n: usize = 0;
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_LENGTH, self.index, &raw mut n as u64),
                0
            );
        }
        n
    }
}

impl arca::Lambda for Ref<Lambda> {
    fn apply(self, argument: associated::AnyValue<Self>) -> associated::Thunk<Self> {
        unsafe {
            assert_eq!(
                syscall(
                    defs::syscall::SYS_APPLY,
                    self.index,
                    self.index,
                    argument.index
                ),
                0
            );
        }
        Ref::new(self.index)
    }

    fn read(self) -> (associated::Thunk<Self>, usize) {
        todo!()
    }
}

impl arca::Thunk for Ref<Thunk> {
    fn run(self) -> associated::AnyValue<Self> {
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

impl arca::AnyValue for Ref<Value> {}

impl<T: ValueType> From<Ref<T>> for Ref<Value> {
    fn from(value: Ref<T>) -> Self {
        Ref::new(value.index)
    }
}

impl Default for Ref<Value> {
    fn default() -> Self {
        os::null().into()
    }
}

pub type DynValue = arca::DynValue<Runtime>;

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

    pub fn tree(values: &mut [Ref<Value>]) -> Ref<Tree> {
        RUNTIME.create_tree(values)
    }

    pub fn prompt() -> Ref<Value> {
        let idx = next_descriptor();
        unsafe {
            assert_eq!(
                syscall(defs::syscall::SYS_RETURN_CONTINUATION_LAMBDA, idx),
                0
            );
        }
        Ref::new(idx)
    }

    pub fn perform<T: Into<Ref<Value>>>(effect: T) -> Ref<Value> {
        let idx = effect.into().index;
        unsafe {
            assert_eq!(syscall(defs::syscall::SYS_PERFORM_EFFECT, idx, idx), 0);
        }
        Ref::new(idx)
    }

    pub fn exit<T: Into<Ref<Value>>>(value: T) -> ! {
        let value = value.into();
        unsafe {
            syscall(defs::syscall::SYS_EXIT, value.index);
            asm!("ud2");
        }
        unreachable!();
    }

    pub fn tailcall(value: Ref<Thunk>) -> ! {
        unsafe {
            syscall(defs::syscall::SYS_TAILCALL, value.index);
            asm!("ud2");
        }
        unreachable!();
    }
}

pub mod prelude {
    pub use super::*;
    pub use arca::{
        Atom as _, Blob as _, DynValue, Error as _, Lambda as _, Null as _, Page as _, Table as _,
        Thunk as _, Tree as _, Word as _,
    };
}
