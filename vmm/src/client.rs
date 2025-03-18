use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::thread::JoinHandle;

use common::message::{MetaRequest, MetaResponse, Request, Response, Type};
use common::ringbuffer::{Endpoint, Error, Receiver, Result, Sender};
use common::BuddyAllocator;
extern crate alloc;

#[derive(Debug)]
enum BufferEntry {
    Ignore,
    Received(Response),
    Waiting(Waker),
}

struct Synchronizer {
    exit: Arc<AtomicBool>,
    channel: async_std::channel::Sender<MetaRequest>,
    buffer: Arc<Mutex<HashMap<usize, BufferEntry>>>,
    send_thread: Mutex<Option<JoinHandle<()>>>,
    receive_thread: Mutex<Option<JoinHandle<()>>>,
}

impl Synchronizer {
    fn new(
        sender: Sender<'static, MetaRequest>,
        receiver: Receiver<'static, MetaResponse>,
    ) -> Self {
        let (tx, rx) = async_std::channel::unbounded();
        let exit = Arc::new(AtomicBool::new(false));
        let exit2 = exit.clone();
        let send_thread = Mutex::new(Some(std::thread::spawn(move || {
            let mut sender = sender;
            let exit = exit2;
            while !exit.load(Ordering::SeqCst) {
                let Ok(request) = rx.recv_blocking() else {
                    break;
                };
                while sender.is_full() {
                    std::thread::yield_now();
                }
                if sender.send(request).is_err() {
                    exit.store(true, Ordering::SeqCst);
                    return;
                }
            }
            sender.hangup();
        })));
        let buffer = Arc::new(Mutex::new(HashMap::new()));
        let buffer2 = buffer.clone();
        let exit2 = exit.clone();
        let receive_thread = Mutex::new(Some(std::thread::spawn(|| {
            let mut receiver = receiver;
            let buffer = buffer2;
            let exit = exit2;
            while !exit.load(Ordering::SeqCst) {
                let response = receiver.try_recv();
                let response = match response {
                    Ok(response) => response,
                    Err(Error::WouldBlock) => {
                        std::thread::yield_now();
                        continue;
                    }
                    Err(_) => {
                        exit.store(true, Ordering::SeqCst);
                        return;
                    }
                };
                let MetaResponse { seqno, body } = response;
                let mut buffer = buffer.lock().unwrap();
                match buffer.remove(&seqno) {
                    Some(BufferEntry::Ignore) => {}
                    Some(BufferEntry::Waiting(waker)) => {
                        buffer.insert(seqno, BufferEntry::Received(body));
                        core::mem::drop(buffer);
                        waker.wake();
                    }
                    Some(BufferEntry::Received(_)) => {
                        unreachable!("received same sequence number twice!");
                    }
                    None => {
                        buffer.insert(seqno, BufferEntry::Received(body));
                    }
                }
            }
            receiver.hangup();
        })));
        Synchronizer {
            exit,
            buffer,
            channel: tx,
            send_thread,
            receive_thread,
        }
    }

    fn ignore(&self, seqno: usize) {
        let mut buffer = self.buffer.lock().unwrap();
        match buffer.remove(&seqno) {
            Some(BufferEntry::Waiting(_)) => {
                unreachable!("ignoring sequence number that already has a waiter");
            }
            Some(BufferEntry::Received(_)) => {}
            Some(BufferEntry::Ignore) | None => {
                buffer.insert(seqno, BufferEntry::Ignore);
            }
        }
    }

    fn get(&self, seqno: usize) -> impl Future<Output = Result<Response>> {
        ClientFuture {
            exit: self.exit.clone(),
            buffer: self.buffer.clone(),
            seqno,
        }
    }

    async fn put(&self, seqno: usize, body: Request) {
        self.channel
            .send(MetaRequest { seqno, body })
            .await
            .unwrap();
    }

    fn put_blocking(&self, seqno: usize, body: Request) {
        self.channel
            .send_blocking(MetaRequest { seqno, body })
            .unwrap();
    }

    fn shutdown(&self) {
        self.channel.close();
        self.exit.store(true, Ordering::SeqCst);
    }
}

struct ClientFuture {
    exit: Arc<AtomicBool>,
    buffer: Arc<Mutex<HashMap<usize, BufferEntry>>>,
    seqno: usize,
}

impl Future for ClientFuture {
    type Output = Result<Response>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut buffer = self.buffer.lock().unwrap();
        if self.exit.load(Ordering::SeqCst) {
            return Poll::Ready(Err(Error::Disconnected));
        }
        match buffer.remove(&self.seqno) {
            Some(BufferEntry::Ignore) => {
                unreachable!(
                    "tried to poll seqno {} which was previously ignored",
                    self.seqno
                );
            }
            Some(BufferEntry::Received(response)) => Poll::Ready(Ok(response)),
            Some(BufferEntry::Waiting(_)) | None => {
                let waker = cx.waker().clone();
                buffer.insert(self.seqno, BufferEntry::Waiting(waker));
                Poll::Pending
            }
        }
    }
}

impl Drop for Synchronizer {
    fn drop(&mut self) {
        self.exit.store(true, Ordering::SeqCst);
        let handle = self.send_thread.lock().unwrap().take().unwrap();
        handle.join().unwrap();
        let handle = self.receive_thread.lock().unwrap().take().unwrap();
        handle.join().unwrap();
    }
}

pub struct Client<'a> {
    synchronizer: Synchronizer,
    seqno: AtomicUsize,
    allocator: &'static BuddyAllocator<'static>,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Client<'a> {
    fn seqno(&self) -> usize {
        self.seqno.fetch_add(1, Ordering::AcqRel)
    }

    pub fn new(endpoint: Endpoint<'static, MetaRequest, MetaResponse>) -> Self {
        let allocator: &'static BuddyAllocator<'static> = endpoint.allocator();
        let (sender, receiver) = endpoint.into_sender_receiver();
        let synchronizer = Synchronizer::new(sender, receiver);
        Client {
            synchronizer,
            seqno: AtomicUsize::new(0),
            allocator,
            _phantom: PhantomData,
        }
    }

    async fn send(&self, message: Request) -> usize {
        let seqno = self.seqno();
        self.synchronizer.put(seqno, message).await;
        seqno
    }

    fn send_blocking(&self, message: Request) -> usize {
        let seqno = self.seqno();
        self.synchronizer.put_blocking(seqno, message);
        seqno
    }

    async fn recv(&self, seqno: usize) -> Response {
        self.synchronizer.get(seqno).await.unwrap()
    }

    async fn fullsend(&self, message: Request) -> Response {
        let seqno = self.send(message).await;
        self.recv(seqno).await
    }

    fn send_and_ignore_blocking(&self, message: Request) {
        let seqno = self.send_blocking(message);
        self.synchronizer.ignore(seqno);
    }

    pub async fn null(&self) -> Handle<Null> {
        let Response::Handle(index) = self.fullsend(Request::CreateNull).await else {
            unreachable!();
        };
        Handle {
            index,
            client: self,
            _phantom: PhantomData,
        }
    }

    pub async fn word(&self, value: u64) -> Handle<Word> {
        let Response::Handle(index) = self.fullsend(Request::CreateWord { value }).await else {
            unreachable!();
        };
        Handle {
            index,
            client: self,
            _phantom: PhantomData,
        }
    }

    pub async fn blob<T: AsRef<[u8]>>(&self, data: T) -> Handle<Blob> {
        let data = data.as_ref();
        let mut blob = Box::new_uninit_slice_in(data.len(), self.allocator);
        blob.write_copy_of_slice(data);
        let blob = unsafe { blob.assume_init() };
        let blob = Arc::from(blob);
        let ptr: *const [u8] = Arc::into_raw(blob);
        let (ptr, len) = ptr.to_raw_parts();
        let ptr = self.allocator.to_offset(ptr);
        let Response::Handle(index) = self.fullsend(Request::CreateBlob { ptr, len }).await else {
            unreachable!();
        };
        Handle {
            index,
            client: self,
            _phantom: PhantomData,
        }
    }

    pub async fn tree<I: IntoIterator<Item = Handle<'a, Opaque>>>(
        &self,
        elements: I,
    ) -> Handle<Tree> {
        let elements = elements.into_iter().map(|x| {
            let index = x.index;
            core::mem::forget(x);
            index
        });
        let mut v = Vec::new_in(self.allocator);
        v.extend(elements);
        let data: Arc<[usize], &BuddyAllocator> = v.into();
        let ptr: *const [usize] = Arc::into_raw(data);
        let (ptr, len) = ptr.to_raw_parts();
        let ptr = self.allocator.to_offset(ptr);
        let Response::Handle(index) = self.fullsend(Request::CreateTree { ptr, len }).await else {
            unreachable!();
        };
        Handle {
            index,
            client: self,
            _phantom: PhantomData,
        }
    }

    pub fn shutdown(&self) {
        self.synchronizer.shutdown();
    }
}

pub struct Null;
pub struct Word;
pub struct Blob;
pub struct Tree;
pub struct Lambda;
pub struct Thunk;
pub struct Opaque;

pub struct Handle<'client, T> {
    client: &'client Client<'client>,
    index: usize,
    _phantom: PhantomData<T>,
}

impl<T> Handle<'_, T> {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl Handle<'_, Word> {
    pub async fn read(&self) -> u64 {
        let Response::Word(x) = self
            .client
            .fullsend(Request::Read { src: self.index })
            .await
        else {
            panic!();
        };
        x
    }
}

impl<'client> Handle<'client, Blob> {
    pub async fn create_thunk(self) -> Handle<'client, Thunk> {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::CreateThunk { src: self.index })
            .await
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        new
    }
}

impl<'client> Handle<'client, Lambda> {
    pub async fn apply(self, arg: Handle<'client, Opaque>) -> Handle<'client, Thunk> {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::Apply {
                src: self.index,
                arg: arg.index,
            })
            .await
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        core::mem::forget(arg);
        new
    }
}

impl<'client> Handle<'client, Thunk> {
    pub async fn run(self) -> Handle<'client, Opaque> {
        let Response::Handle(index) = self.client.fullsend(Request::Run { src: self.index }).await
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        new
    }
}

impl<T> Handle<'_, T> {
    pub async fn duplicate(&self) -> Self {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::Clone { src: self.index })
            .await
        else {
            unreachable!();
        };
        Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        }
    }
}

impl<'a> From<Handle<'a, Word>> for Handle<'a, Opaque> {
    fn from(value: Handle<'a, Word>) -> Handle<'a, Opaque> {
        let handle = Handle {
            client: value.client,
            index: value.index,
            _phantom: PhantomData,
        };
        core::mem::forget(value);
        handle
    }
}

impl<'a> From<Handle<'a, Tree>> for Handle<'a, Opaque> {
    fn from(value: Handle<'a, Tree>) -> Handle<'a, Opaque> {
        let handle = Handle {
            client: value.client,
            index: value.index,
            _phantom: PhantomData,
        };
        core::mem::forget(value);
        handle
    }
}

impl<'a> Handle<'a, Opaque> {
    pub async fn as_word(self) -> core::result::Result<Handle<'a, Word>, Handle<'a, Opaque>> {
        let datatype = self
            .client
            .fullsend(Request::GetType { src: self.index })
            .await;
        if let Response::Type(Type::Word) = datatype {
            let handle = Handle {
                client: self.client,
                index: self.index,
                _phantom: PhantomData,
            };
            core::mem::forget(self);
            Ok(handle)
        } else {
            log::error!("{datatype:?}");
            Err(self)
        }
    }

    pub async fn as_lambda(self) -> core::result::Result<Handle<'a, Lambda>, Handle<'a, Opaque>> {
        let datatype = self
            .client
            .fullsend(Request::GetType { src: self.index })
            .await;
        if let Response::Type(Type::Lambda) = datatype {
            let handle = Handle {
                client: self.client,
                index: self.index,
                _phantom: PhantomData,
            };
            core::mem::forget(self);
            Ok(handle)
        } else {
            Err(self)
        }
    }
}

impl<T> Drop for Handle<'_, T> {
    fn drop(&mut self) {
        self.client
            .send_and_ignore_blocking(Request::Drop { src: self.index });
    }
}
