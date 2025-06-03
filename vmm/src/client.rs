use std::future::Future;
use std::marker::PhantomData;
use std::num::NonZero;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::JoinHandle;

pub use arca::{
    Atom as _, Blob as _, DataType, Error as _, Lambda as _, Null as _, Page as _, Runtime as _,
    Table as _, Thunk as _, Tree as _, Value as _, ValueType, Word as _,
};

use common::message::{self, *};
use common::ringbuffer::{self, Error as RingBufferError, Receiver, Result, Sender};
use common::BuddyAllocator;

use crate::runtime::Runtime;
extern crate alloc;

const SERVER_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_server"));

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
                    context,
                    body,
                } = response;
                let waker: Arc<Waker> = unsafe { Arc::from_raw(function as *const _) };
                let result: Arc<Mutex<Option<Response>>> =
                    unsafe { Arc::from_raw(context as *const _) };
                let mut option = result.lock().unwrap();
                *option = Some(body);
                waker.wake_by_ref();
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

    fn send(&self, body: Request) -> ClientFuture {
        ClientFuture {
            sender: self.sender.clone(),
            body: Mutex::new(Some(body)),
            result: Arc::new(Mutex::new(None)),
        }
    }

    fn shutdown(&self) {
        self.exit.store(true, Ordering::SeqCst);
        let sender = self.sender.lock().unwrap();
        sender.hangup();
    }
}

struct ClientFuture {
    sender: Arc<Mutex<Sender<MetaRequest>>>,
    body: Mutex<Option<Request>>,
    result: Arc<Mutex<Option<Response>>>,
}

impl Future for ClientFuture {
    type Output = Result<Response>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut result = self.result.lock().unwrap();
        if let Some(result) = result.take() {
            Poll::Ready(Ok(result))
        } else {
            let mut sender = self.sender.lock().unwrap();
            let waker = cx.waker();
            let waker = Arc::new(waker.clone());
            let waker = Arc::into_raw(waker);
            let result = self.result.clone();
            let result = Arc::into_raw(result);
            let mut body = self.body.lock().unwrap();
            if let Err(e) = sender.send(MetaRequest {
                function: waker as usize,
                context: result as usize,
                body: body.take().unwrap(),
            }) {
                Poll::Ready(Err(e))
            } else {
                Poll::Pending
            }
        }
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
        let ep2 = BuddyAllocator.to_offset(Box::into_raw(Box::new_in(
            ep2.into_raw_parts(),
            BuddyAllocator,
        )));

        std::thread::spawn(move || {
            runtime.run(&[ep2]);
        });

        let (sender, receiver) = ep1.into_sender_receiver();
        let synchronizer = Synchronizer::new(sender, receiver);
        Client { synchronizer }
    }

    fn fullsend(&self, message: Request) -> Result<Response> {
        let f = self.synchronizer.send(message);
        core::mem::drop(f);
        todo!();
    }

    pub fn shutdown(&self) {
        self.synchronizer.shutdown();
    }

    fn new_ref<T>(self: &Arc<Self>, handle: Handle) -> Ref<T> {
        Ref {
            client: self.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }
}

pub struct ArcaRuntime {
    client: Arc<Client>,
}

impl ArcaRuntime {
    pub fn new(memory: usize) -> Self {
        let client = Client::new(memory);
        Self {
            client: Arc::new(client),
        }
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
            .fullsend(Request::CreateError(value.handle.take().unwrap()))
        else {
            panic!("could not create Error");
        };
        Self::Error {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_atom(&self, data: &[u8]) -> Self::Atom {
        let ptr = BuddyAllocator.to_offset(data.as_ptr());
        let len = data.len();
        let Ok(Response::Handle(handle)) = self.client.fullsend(Request::CreateAtom { ptr, len })
        else {
            panic!("could not create Atom");
        };
        Self::Atom {
            client: self.client.clone(),
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    fn create_blob(&self, data: &[u8]) -> Self::Blob {
        let mut v = Vec::new_in(BuddyAllocator);
        v.extend_from_slice(data);
        let data = v.into_boxed_slice();
        let ptr = BuddyAllocator.to_offset(data.as_ptr());
        let len = data.len();
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
            thunk: thunk.handle.take().unwrap(),
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
            registers: registers.handle.take().unwrap(),
            memory: memory.handle.take().unwrap(),
            descriptors: descriptors.handle.take().unwrap(),
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

impl<T> arca::RuntimeType for Ref<T> {
    type Runtime = ArcaRuntime;
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
        todo!()
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
    fn read(&self, _buffer: &mut [u8]) {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}

impl arca::Tree for Ref<Tree> {
    fn take(&mut self, _index: usize) -> arca::associated::Value<Self> {
        todo!()
    }

    fn put(
        &mut self,
        _index: usize,
        _value: arca::associated::Value<Self>,
    ) -> arca::associated::Value<Self> {
        todo!()
    }

    fn len(&self) -> usize {
        todo!()
    }
}

impl arca::Page for Ref<Page> {
    fn read(&self, _offset: usize, _buffer: &mut [u8]) {
        todo!()
    }

    fn write(&mut self, _offset: usize, _buffer: &[u8]) {
        todo!()
    }

    fn size(&self) -> usize {
        todo!()
    }
}

impl arca::Table for Ref<Table> {
    fn take(&mut self, _index: usize) -> arca::Entry<Self> {
        todo!()
    }

    fn put(
        &mut self,
        _index: usize,
        _entry: arca::Entry<Self>,
    ) -> std::result::Result<arca::Entry<Self>, arca::Entry<Self>> {
        todo!()
    }

    fn size(&self) -> usize {
        todo!()
    }
}

impl arca::Lambda for Ref<Lambda> {
    fn apply(self, _argument: arca::associated::Value<Self>) -> arca::associated::Thunk<Self> {
        todo!()
    }

    fn read(self) -> (arca::associated::Thunk<Self>, usize) {
        todo!()
    }
}

impl arca::Thunk for Ref<Thunk> {
    fn run(self) -> arca::associated::Value<Self> {
        todo!()
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
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        unsafe {
            let handle = self.handle.as_ref().unwrap().copy();
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
            Ok(value.client.new_ref(value.handle.take().unwrap()))
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
        value.client.new_ref(value.handle.take().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));

    #[test]
    pub fn test_client() {
        let client = ArcaRuntime::new(1 << 32);
        let _add = client.create_blob(ADD_ELF);
        let _add: Ref<Thunk> = todo!();
        // let x = client.create_word(1);
        // let y = client.create_word(2);
        // let mut arg = client.create_tree(2);
        // arg.put(0, x.into());
        // arg.put(1, x.into());
        // let add: Ref<Lambda> = add.run().try_into().unwrap();
        // let sum: Ref<Word> = add.apply(arg.into()).run().try_into().unwrap();
        // assert_eq!(sum.read(), 3);
    }
}
