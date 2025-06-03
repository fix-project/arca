use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::JoinHandle;

use common::message::{self, *};
use common::ringbuffer::{Endpoint, Error as RingBufferError, Receiver, Result, Sender};
use common::BuddyAllocator;
extern crate alloc;

struct Synchronizer {
    exit: Arc<AtomicBool>,
    sender: Arc<Mutex<Sender<'static, MetaRequest>>>,
    receive_thread: Mutex<Option<JoinHandle<()>>>,
}

impl Synchronizer {
    fn new(
        sender: Sender<'static, MetaRequest>,
        receiver: Receiver<'static, MetaResponse>,
    ) -> Self {
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
    sender: Arc<Mutex<Sender<'static, MetaRequest>>>,
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

pub struct Client<'a> {
    synchronizer: Synchronizer,
    allocator: &'static BuddyAllocator<'static>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Client<'a> {
    pub fn new(endpoint: Endpoint<'static, MetaRequest, MetaResponse>) -> Self {
        let allocator: &'static BuddyAllocator<'static> = endpoint.allocator();
        let (sender, receiver) = endpoint.into_sender_receiver();
        let synchronizer = Synchronizer::new(sender, receiver);
        Client {
            synchronizer,
            allocator,
            _phantom: PhantomData,
        }
    }

    async fn fullsend(&self, message: Request) -> Result<Response> {
        self.synchronizer.send(message).await
    }

    pub async fn null(&'a self) -> Ref<'a, Null> {
        Ref {
            client: self,
            handle: Some(Handle::null()),
            _phantom: PhantomData,
        }
    }

    pub async fn word(&'a self, value: u64) -> Ref<'a, Word> {
        Ref {
            client: self,
            handle: Some(Handle::word(value)),
            _phantom: PhantomData,
        }
    }

    pub async fn error<T: Into<Ref<'a, Value>>>(&'a self, data: T) -> Ref<'a, Error> {
        let mut data: Ref<Value> = data.into();
        let Response::Handle(
            handle @ message::Handle {
                datatype: Type::Error,
                parts: _,
            },
        ) = self
            .fullsend(Request::CreateError(data.handle.take().unwrap()))
            .await
            .unwrap()
        else {
            unreachable!();
        };

        Ref {
            client: self,
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    pub async fn blob<T: AsRef<[u8]>>(&self, data: T) -> Ref<Blob> {
        let data = data.as_ref();
        let mut blob = Box::new_uninit_slice_in(data.len(), self.allocator);
        blob.write_copy_of_slice(data);
        let blob = unsafe { blob.assume_init() };
        let blob = Arc::from(blob);
        let ptr: *const [u8] = Arc::into_raw(blob);
        let (ptr, len) = ptr.to_raw_parts();
        let ptr = self.allocator.to_offset(ptr);

        let Response::Handle(
            handle @ message::Handle {
                datatype: Type::Blob,
                parts: _,
            },
        ) = self
            .fullsend(Request::CreateBlob { ptr, len })
            .await
            .unwrap()
        else {
            unreachable!();
        };

        Ref {
            client: self,
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    pub async fn tree<I: IntoIterator<Item = Ref<'a, Value>>>(
        &'a self,
        elements: I,
    ) -> Ref<'a, Tree> {
        let elements = elements
            .into_iter()
            .inspect(|x| assert_eq!(self as *const _, x.client as *const _))
            .map(|mut x| x.handle.take().unwrap());
        let mut v = Vec::new_in(self.allocator);
        v.extend(elements);
        let data: Box<[Handle], &BuddyAllocator> = v.into();
        let ptr: *const [Handle] = Box::into_raw(data);
        let (ptr, len) = ptr.to_raw_parts();
        let ptr = self.allocator.to_offset(ptr);

        let Response::Handle(
            handle @ message::Handle {
                datatype: Type::Tree,
                parts: _,
            },
        ) = self
            .fullsend(Request::CreateTree { ptr, len })
            .await
            .unwrap()
        else {
            unreachable!();
        };

        Ref {
            client: self,
            handle: Some(handle),
            _phantom: PhantomData,
        }
    }

    pub fn shutdown(&self) {
        self.synchronizer.shutdown();
    }
}

pub struct Ref<'client, T> {
    client: &'client Client<'client>,
    handle: Option<message::Handle>,
    _phantom: PhantomData<T>,
}

impl<T> arca::RuntimeType for Ref<T> {
    type Runtime = Client;
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

pub struct Value;

impl arca::Null for Ref<Null> {}

impl arca::Word for Ref<Word> {
    fn read(&self) -> u64 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use common::ringbuffer;

    use crate::runtime::{Mmap, Runtime};

    use super::*;

    const SERVER_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_server"));
    const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));

    #[test]
    pub fn test_client() {
        let mut mmap = Mmap::new(1 << 32);
        let runtime = Runtime::new(1, &mut mmap, SERVER_ELF.into());
        let allocator = runtime.allocator();
        let a: &'static BuddyAllocator<'static> = unsafe { core::mem::transmute(&*allocator) };
        let (endpoint1, endpoint2) = ringbuffer::pair(1024, a);
        let endpoint_raw = Box::into_raw(Box::new_in(
            endpoint2.into_raw_parts(&allocator),
            &*allocator,
        ));
        let endpoint_raw_offset = allocator.to_offset(endpoint_raw);

        let client = Client::new(endpoint1);

        std::thread::scope(|s| {
            s.spawn(|| {
                runtime.run(&[endpoint_raw_offset]);
            });

            async_std::task::block_on(async {
                let add = client.blob(ADD_ELF).await;
                let add = add.create_thunk().await;
                let x = client.word(1).await;
                let y = client.word(2).await;
                let arg = client.tree([x.into(), y.into()]).await;
                let DynRef::Lambda(add) = add.run().await.into() else {
                    panic!("running add returned something other than a lambda");
                };
                let DynRef::Word(sum) = add.apply(arg).await.run().await.into() else {
                    panic!("running add(1, 2) returned something other than a word");
                };
                assert_eq!(sum.read().await, 3);
            });
            client.shutdown();
        });
    }

    #[test]
    pub fn test_rw() {
        let mut mmap = Mmap::new(1 << 32);
        let runtime = Runtime::new(1, &mut mmap, SERVER_ELF.into());
        let allocator = runtime.allocator();
        let a: &'static BuddyAllocator<'static> = unsafe { core::mem::transmute(&*allocator) };
        let (endpoint1, endpoint2) = ringbuffer::pair(1024, a);
        let endpoint_raw = Box::into_raw(Box::new_in(
            endpoint2.into_raw_parts(&allocator),
            &*allocator,
        ));
        let endpoint_raw_offset = allocator.to_offset(endpoint_raw);

        let client = Client::new(endpoint1);

        std::thread::scope(|s| {
            s.spawn(|| {
                runtime.run(&[endpoint_raw_offset]);
            });

            async_std::task::block_on(async {
                let blob = client.blob("hello").await;
                assert_eq!(blob.read().await, b"hello");

                let tree = client
                    .tree([client.word(10).await.into(), client.word(20).await.into()])
                    .await;
                let contents = tree.read().await;
                assert_eq!(contents.len(), 2);
                let DynRef::Word(word) = contents[0].clone().into() else {
                    panic!();
                };
                assert_eq!(word.read().await, 10);
                let DynRef::Word(word) = contents[1].clone().into() else {
                    panic!();
                };
                assert_eq!(word.read().await, 20);

                let error = client.error(client.blob("error").await).await;
                let contents = error.read().await;
                let DynRef::Blob(blob) = contents.into() else {
                    panic!();
                };
                assert_eq!(blob.read().await, b"error");
            });
            client.shutdown();
        });
    }
}
