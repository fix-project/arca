use std::marker::PhantomData;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

pub use arca::prelude::*;

use common::message::{self, *};
use common::ringbuffer::{self, Error as RingBufferError, Receiver, Result, Sender};
use common::util::initcell::LazyLock;
use common::BuddyAllocator;

use crate::runtime::Runtime;
extern crate alloc;

const SERVER_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_server"));

static RUNTIME: LazyLock<ArcaRuntime> = LazyLock::new(|| ArcaRuntime::new(1 << 30));

pub fn init(size: usize) {
    LazyLock::set(&RUNTIME, ArcaRuntime::new(size)).expect("runtime was already initialized");
}

pub fn wait() {
    LazyLock::wait(&RUNTIME);
}

pub fn runtime() -> &'static ArcaRuntime {
    &RUNTIME
}

struct SyncFlag<T> {
    flag: Mutex<Option<T>>,
    condvar: Condvar,
}

impl<T> SyncFlag<T> {
    pub fn new() -> Self {
        Self {
            flag: Mutex::new(None),
            condvar: Condvar::new(),
        }
    }

    pub fn wait(&self) -> T {
        let mutex = self.flag.lock().unwrap();
        let mut mutex = self
            .condvar
            .wait_while(mutex, |flag: &mut Option<T>| flag.is_none())
            .unwrap();
        mutex.take().unwrap()
    }

    pub fn notify(&self, value: T) {
        let mut mutex = self.flag.lock().unwrap();
        *mutex = Some(value);
        self.condvar.notify_one();
    }
}

struct Synchronizer {
    exit: Arc<AtomicBool>,
    sender: Arc<Mutex<Sender<MetaRequest>>>,
    receive_thread: Mutex<Option<JoinHandle<()>>>,
}

impl Synchronizer {
    fn new(sender: Sender<MetaRequest>, receiver: Receiver<MetaResponse>) -> Self {
        let exit = Arc::new(AtomicBool::new(false));
        let exit2 = exit.clone();
        let receive_thread = Mutex::new(Some(std::thread::spawn(|| {
            let mut receiver = receiver;
            let exit = exit2;
            while !exit.load(Ordering::SeqCst) {
                let response = receiver.try_recv();
                let response = match response {
                    Ok(response) => response,
                    Err(RingBufferError::WouldBlock) => {
                        std::thread::yield_now();
                        continue;
                    }
                    Err(_) => {
                        exit.store(true, Ordering::SeqCst);
                        return;
                    }
                };
                let MetaResponse {
                    function,
                    context: _,
                    body,
                } = response;
                let sync: Arc<SyncFlag<Response>> = unsafe { Arc::from_raw(function as *const _) };
                sync.notify(body);
            }
            receiver.hangup();
        })));
        let sender = Arc::new(Mutex::new(sender));
        Synchronizer {
            exit,
            sender,
            receive_thread,
        }
    }

    fn send(&self, body: Request) -> Response {
        let sync: Arc<SyncFlag<Response>> = Arc::new(SyncFlag::new());
        let mut sender = self.sender.lock().unwrap();
        sender
            .send(MetaRequest {
                function: Arc::into_raw(sync.clone()) as usize,
                context: 0,
                body,
            })
            .unwrap();
        sync.wait()
    }

    fn shutdown(&self) {
        self.exit.store(true, Ordering::SeqCst);
        let sender = self.sender.lock().unwrap();
        sender.hangup();
    }
}

impl Drop for Synchronizer {
    fn drop(&mut self) {
        self.exit.store(true, Ordering::SeqCst);
        let handle = self.receive_thread.lock().unwrap().take().unwrap();
        handle.join().unwrap();
    }
}

pub struct Client {
    synchronizer: Synchronizer,
}

impl Client {
    pub fn new(ram: usize) -> Self {
        let cores = std::thread::available_parallelism().unwrap_or(NonZero::new(1).unwrap());
        let runtime = Runtime::new(cores.into(), ram, SERVER_ELF.into());
        let (ep1, ep2) = ringbuffer::pair(1024);
        let ep2 = BuddyAllocator.to_offset(
            Box::into_raw_with_allocator(Box::new_in(ep2.into_raw_parts(), BuddyAllocator)).0,
        );

        std::thread::spawn(move || {
            runtime.run(&[ep2]);
        });

        let (sender, receiver) = ep1.into_sender_receiver();
        let synchronizer = Synchronizer::new(sender, receiver);
        Client { synchronizer }
    }

    fn fullsend(&self, message: Request) -> Result<Response> {
        Ok(self.synchronizer.send(message))
    }

    fn request_handle(&self, message: Request) -> Handle {
        let Response::Handle(h) = self.fullsend(message).unwrap() else {
            panic!("expected handle");
        };
        h
    }

    fn request_ref<T>(self: &Arc<Self>, message: Request) -> Ref<T>
    where
        Ref<T>: arca::ValueType,
    {
        let handle = self.request_handle(message);
        self.new_ref(handle)
    }

    fn request_value(self: &Arc<Self>, message: Request) -> Ref<Value> {
        let handle = self.request_handle(message);
        self.new_value_ref(handle)
    }

    fn request_span(self: &Arc<Self>, message: Request) -> *mut [u8] {
        let Response::Span { ptr, len } = self.fullsend(message).unwrap() else {
            panic!("expected span");
        };
        core::ptr::from_raw_parts_mut(BuddyAllocator.from_offset::<u8>(ptr), len)
    }

    fn request_length(&self, message: Request) -> usize {
        let Response::Length(n) = self.fullsend(message).unwrap() else {
            panic!("expected length");
        };
        n
    }

    pub fn shutdown(&self) {
        self.synchronizer.shutdown();
    }

    fn new_ref<T>(self: &Arc<Self>, handle: Handle) -> Ref<T>
    where
        Ref<T>: arca::ValueType,
    {
        assert_eq!(handle.datatype(), Ref::<T>::DATATYPE);
        Ref {
            client: self.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn new_value_ref(self: &Arc<Self>, handle: Handle) -> Ref<Value> {
        Ref {
            client: self.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }
}

#[derive(Debug)]
pub struct ArcaRuntime {
    client: Arc<Client>,
}

impl ArcaRuntime {
    fn new(memory: usize) -> Self {
        let client = Client::new(memory);
        Self {
            client: Arc::new(client),
        }
    }

    pub fn load_elf(&self, elf: &[u8]) -> Ref<Thunk> {
        common::elfloader::load_elf(self, elf)
    }
}

impl arca::Runtime for ArcaRuntime {
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
        Self::Null {
            client: self.client.clone(),
            handle: Some(Handle::null()),
            _phantom: PhantomData,
        }
    }

    fn create_word(&self, value: u64) -> Self::Word {
        Self::Word {
            client: self.client.clone(),
            handle: Some(Handle::word(value)),
            _phantom: PhantomData,
        }
    }

    fn create_error(&self, mut value: Self::Value) -> Self::Error {
        let Ok(Response::Handle(handle)) = self
            .client
            .fullsend(Request::CreateError(value.take_handle()))
        else {
            panic!("could not create Error");
        };
        Self::Error {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_atom(&self, _data: &[u8]) -> Self::Atom {
        todo!();
        // let ptr = BuddyAllocator.to_offset(data.as_ptr());
        // let len = data.len();
        // let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateAtom { ptr, len })
        // else {
        //     panic!("could not create Atom");
        // };
        // Self::Atom {
        //     client: self.client.clone(),
        //     handle: Some(handle),
        //     _phantom: PhantomData,
        // }
    }

    fn create_blob(&self, data: &[u8]) -> Self::Blob {
        let mut v = Vec::new_in(BuddyAllocator);
        v.extend_from_slice(data);
        let data = v.into_boxed_slice();
        let (ptr, len) = Box::into_raw_with_allocator(data).0.to_raw_parts();
        let ptr = BuddyAllocator.to_offset(ptr);
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateBlob { ptr, len })
        else {
            panic!("could not create Blob");
        };
        Self::Blob {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_tree(&self, size: usize) -> Self::Tree {
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateTree { size })
        else {
            panic!("could not create Tree");
        };
        Self::Tree {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_page(&self, size: usize) -> Self::Page {
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreatePage { size })
        else {
            panic!("could not create Page");
        };
        Self::Page {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_table(&self, size: usize) -> Self::Table {
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateTable { size })
        else {
            panic!("could not create Table");
        };
        Self::Table {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_lambda(&self, mut thunk: Self::Thunk, index: usize) -> Self::Lambda {
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateLambda {
            thunk: thunk.take_handle(),
            index,
        }) else {
            panic!("could not create Lambda");
        };
        Self::Lambda {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_thunk(
        &self,
        mut registers: Self::Blob,
        mut memory: Self::Table,
        mut descriptors: Self::Tree,
    ) -> Self::Thunk {
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateThunk {
            registers: registers.take_handle(),
            memory: memory.take_handle(),
            descriptors: descriptors.take_handle(),
        }) else {
            panic!("could not create Lambda");
        };
        Self::Thunk {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }
}

impl core::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Client")
    }
}

#[derive(Debug)]
pub struct Ref<T> {
    client: Arc<Client>,
    handle: Option<message::Handle>,
    _phantom: PhantomData<T>,
}

impl<T> Ref<T> {
    fn take_handle(&mut self) -> message::Handle {
        self.handle.take().unwrap()
    }

    unsafe fn copy_handle(&self) -> message::Handle {
        self.handle.as_ref().unwrap().copy()
    }
}

impl<T> From<Ref<T>> for message::Handle {
    fn from(mut value: Ref<T>) -> Self {
        value.handle.take().unwrap()
    }
}

impl<T> arca::RuntimeType for Ref<T> {
    type Runtime = ArcaRuntime;

    fn runtime(&self) -> &Self::Runtime {
        &RUNTIME
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

impl arca::Null for Ref<Null> {}

impl arca::Word for Ref<Word> {
    fn read(&self) -> u64 {
        self.handle.as_ref().unwrap().get_word().unwrap()
    }
}

impl arca::Error for Ref<Error> {
    fn read(self) -> arca::associated::Value<Self> {
        todo!()
    }
}

impl arca::Atom for Ref<Atom> {}

impl PartialEq for Ref<Atom> {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Eq for Ref<Atom> {}

impl arca::Blob for Ref<Blob> {
    fn read(&self, buffer: &mut [u8]) {
        unsafe {
            let span = self
                .client
                .request_span(Request::ReadBlob(self.copy_handle()));
            buffer.copy_from_slice(&*span);
        }
    }

    fn len(&self) -> usize {
        unsafe {
            let span = self
                .client
                .request_span(Request::ReadBlob(self.copy_handle()));
            span.len()
        }
    }
}

impl arca::Tree for Ref<Tree> {
    fn take(&mut self, _index: usize) -> arca::associated::Value<Self> {
        todo!()
    }

    fn put(
        &mut self,
        index: usize,
        mut value: arca::associated::Value<Self>,
    ) -> arca::associated::Value<Self> {
        self.client.request_value(Request::TreePut(
            unsafe { self.copy_handle() },
            index,
            value.take_handle(),
        ))
    }

    fn get(&self, _index: usize) -> arca::associated::Value<Self> {
        todo!()
    }

    fn set(&mut self, _index: usize, _value: arca::associated::Value<Self>) {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}

impl arca::Page for Ref<Page> {
    fn read(&self, offset: usize, buffer: &mut [u8]) {
        unsafe {
            let span = self
                .client
                .request_span(Request::ReadPage(self.copy_handle()));
            buffer.copy_from_slice(&(&*span)[offset..]);
        }
    }

    fn write(&mut self, offset: usize, buffer: &[u8]) {
        let mut v = Vec::new_in(BuddyAllocator);
        v.extend_from_slice(buffer);
        let data = v.into_boxed_slice();
        let (ptr, len) = Box::into_raw_with_allocator(data).0.to_raw_parts();
        let ptr = BuddyAllocator.to_offset(ptr);
        let handle = self.take_handle();
        let handle = self.client.request_handle(Request::WritePage {
            handle,
            offset,
            ptr,
            len,
        });
        self.handle = Some(handle)
    }

    fn size(&self) -> usize {
        unsafe {
            let span = self
                .client
                .request_span(Request::ReadPage(self.copy_handle()));
            span.len()
        }
    }
}

impl arca::Table for Ref<Table> {
    fn take(&mut self, index: usize) -> arca::Entry<Self> {
        assert!(index < 512);
        let Response::Entry(entry) = self
            .client
            .fullsend(Request::TableTake(unsafe { self.copy_handle() }, index))
            .unwrap()
        else {
            unreachable!();
        };

        match entry {
            Entry::Null(size) => arca::Entry::Null(size),
            Entry::ReadOnly(value) if value.datatype() == DataType::Page => {
                arca::Entry::ROPage(self.client.new_ref(value))
            }
            Entry::ReadWrite(value) if value.datatype() == DataType::Page => {
                arca::Entry::RWPage(self.client.new_ref(value))
            }
            Entry::ReadOnly(value) if value.datatype() == DataType::Table => {
                arca::Entry::ROTable(self.client.new_ref(value))
            }
            Entry::ReadWrite(value) if value.datatype() == DataType::Table => {
                arca::Entry::RWTable(self.client.new_ref(value))
            }
            _ => unreachable!(),
        }
    }

    fn put(
        &mut self,
        index: usize,
        entry: arca::Entry<Self>,
    ) -> std::result::Result<arca::Entry<Self>, arca::Entry<Self>> {
        assert!(index < 512);
        let Response::Entry(entry) = self
            .client
            .fullsend(Request::TablePut(
                unsafe { self.copy_handle() },
                index,
                match entry {
                    arca::Entry::Null(x) => Entry::Null(x),
                    arca::Entry::ROPage(page) => Entry::ReadOnly(page.into()),
                    arca::Entry::RWPage(page) => Entry::ReadWrite(page.into()),
                    arca::Entry::ROTable(table) => Entry::ReadOnly(table.into()),
                    arca::Entry::RWTable(table) => Entry::ReadWrite(table.into()),
                },
            ))
            .unwrap()
        else {
            unreachable!();
        };
        let entry = match entry {
            Entry::Null(size) => arca::Entry::Null(size),
            Entry::ReadOnly(value) if value.datatype() == DataType::Page => {
                arca::Entry::ROPage(self.client.new_ref(value))
            }
            Entry::ReadWrite(value) if value.datatype() == DataType::Page => {
                arca::Entry::RWPage(self.client.new_ref(value))
            }
            Entry::ReadOnly(value) if value.datatype() == DataType::Table => {
                arca::Entry::ROTable(self.client.new_ref(value))
            }
            Entry::ReadWrite(value) if value.datatype() == DataType::Table => {
                arca::Entry::RWTable(self.client.new_ref(value))
            }
            _ => unreachable!(),
        };
        Ok(entry)
    }

    fn get(&mut self, _index: usize) -> arca::Entry<Self> {
        todo!()
    }

    fn set(
        &mut self,
        _index: usize,
        _entry: arca::Entry<Self>,
    ) -> std::result::Result<(), arca::Entry<Self>> {
        todo!()
    }

    fn size(&self) -> usize {
        unsafe {
            self.client
                .request_length(Request::Length(self.copy_handle()))
        }
    }
}

impl arca::Lambda for Ref<Lambda> {
    fn read(self) -> (arca::associated::Thunk<Self>, usize) {
        todo!()
    }
}

impl arca::Thunk for Ref<Thunk> {
    fn run(mut self) -> arca::associated::Value<Self> {
        let handle = self.take_handle();
        self.client.request_value(Request::Run(handle))
    }

    fn read(
        self,
    ) -> (
        arca::associated::Blob<Self>,
        arca::associated::Table<Self>,
        arca::associated::Tree<Self>,
    ) {
        todo!()
    }
}

impl arca::Value for Ref<Value> {
    fn datatype(&self) -> DataType {
        self.handle.as_ref().unwrap().datatype()
    }

    fn apply(
        mut self,
        mut argument: arca::associated::Value<Self>,
    ) -> arca::associated::Thunk<Self> {
        let handle = self.take_handle();
        self.client
            .request_ref(Request::Apply(handle, argument.take_handle()))
    }
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        unsafe {
            let handle = self.copy_handle();
            let Ok(Response::Handle(handle)) = self.client.fullsend(Request::Clone(handle)) else {
                panic!("could not clone handle");
            };
            Self {
                client: self.client.clone(),
                handle: Some(handle),
                _phantom: PhantomData,
            }
        }
    }
}

impl<T> Drop for Ref<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let Ok(Response::Ack) = self.client.fullsend(Request::Drop(handle)) else {
                panic!("could not drop handle");
            };
        }
    }
}

impl<T> TryFrom<Ref<Value>> for Ref<T>
where
    Ref<T>: ValueType,
{
    type Error = Ref<Value>;

    fn try_from(mut value: Ref<Value>) -> std::result::Result<Self, Self::Error> {
        if value.datatype() == Ref::<T>::DATATYPE {
            let handle = value.take_handle();
            Ok(value.client.new_ref(handle))
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
        let handle = value.take_handle();
        value.client.new_value_ref(handle)
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::Bencher;

    const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
    const NULL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_null"));

    #[test]
    pub fn test_client() {
        let arca = runtime();
        let add = arca.load_elf(ADD_ELF);
        let add: Ref<Lambda> = add.run().try_into().unwrap();
        let x = arca.create_word(1);
        let y = arca.create_word(2);
        let mut arg = arca.create_tree(2);
        arg.put(0, x.into());
        arg.put(1, y.into());
        let sum: Ref<Word> = add.apply(arg).run().try_into().unwrap();
        assert_eq!(sum.read(), 3);
    }

    #[bench]
    pub fn bench_client(b: &mut Bencher) {
        let arca = runtime();
        let null = arca.load_elf(NULL_ELF);
        b.iter(|| {
            let null = null.clone();
            let _: Ref<Null> = null.run().try_into().unwrap();
        });
    }
}
