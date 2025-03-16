use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Waker, Context, Poll};
use std::thread::JoinHandle;
use std::future::Future;
use std::pin::Pin;

use common::message::{MetaRequest, MetaResponse, Request, Response};
use common::ringbuffer::{Endpoint, Receiver, Result, Sender, Error};
use common::BuddyAllocator;
extern crate alloc;

enum BufferEntry {
    Ignore,
    Received(Response),
    Waiting(Waker),
}

struct Synchronizer {
    exit: Arc<AtomicBool>,
    buffer: Arc<Mutex<HashMap<usize, BufferEntry>>>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl Synchronizer {
    fn new(receiver: Receiver<'static, MetaResponse>) -> Self {
        let buffer = Arc::new(Mutex::new(HashMap::new()));
        let buffer2 = buffer.clone();
        let exit = Arc::new(AtomicBool::new(false));
        let exit2 = exit.clone();
        let thread = Mutex::new(Some(std::thread::spawn(|| {
            let mut receiver = receiver;
            let buffer = buffer2;
            let exit = exit2;
            while !exit.load(Ordering::SeqCst) {
                let response = receiver.try_recv();
                let response = match response {
                    Ok(response) => response,
                    Err(Error::WouldBlock) => {
                        core::hint::spin_loop();
                        continue;
                    }
                    Err(_) => {
                        exit.store(true, Ordering::SeqCst);
                        return;
                    }
                };
                let MetaResponse{seqno, body} = response;
                let mut buffer = buffer.lock().unwrap();
                match buffer.remove(&seqno) {
                    Some(BufferEntry::Ignore) => {},
                    Some(BufferEntry::Waiting(waker)) => {
                        buffer.insert(seqno, BufferEntry::Received(body));
                        waker.wake();
                    },
                    Some(BufferEntry::Received(_)) => {
                        unreachable!("received same sequence number twice!");
                    },
                    None => {
                        buffer.insert(seqno, BufferEntry::Received(body));
                    }
                }
            }
        })));
        Synchronizer {
            exit,
            buffer,
            thread,
        }
    }

    fn ignore(&self, seqno: usize) {
        let mut buffer = self.buffer.lock().unwrap();
        match buffer.remove(&seqno) {
            Some(BufferEntry::Waiting(_)) => {
                unreachable!("ignoring sequence number that already has a waiter");
            },
            Some(BufferEntry::Received(_)) => {},
            Some(BufferEntry::Ignore) | None => {
                buffer.insert(seqno, BufferEntry::Ignore);
            }
        }
    }

    fn get(&self, seqno: usize) -> impl Future<Output=Result<Response>> {
        ClientFuture {
            exit: self.exit.clone(),
            buffer: self.buffer.clone(),
            seqno,
        }
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
        if !buffer.contains_key(&self.seqno) {
            let waker = cx.waker().clone();
            buffer.insert(self.seqno, BufferEntry::Waiting(waker));
            return Poll::Pending;
        }
        let BufferEntry::Received(data) = buffer.remove(&self.seqno).unwrap() else {
            unreachable!("tried to poll seqno with no data");
        };
        Poll::Ready(Ok(data))
    }
}

impl Drop for Synchronizer {
    fn drop(&mut self) {
        self.exit.store(true, Ordering::SeqCst);
        let handle = self.thread.lock().unwrap().take().unwrap();
        handle.join().unwrap();
    }
}

pub struct Client<'a> {
    synchronizer: Synchronizer,
    seqno: AtomicUsize,
    sender: Mutex<Sender<'static, MetaRequest>>,
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
        let synchronizer = Synchronizer::new(receiver);
        Client {
            synchronizer,
            seqno: AtomicUsize::new(0),
            sender: Mutex::new(sender),
            allocator,
            _phantom: PhantomData,
        }
    }

    fn send(&self, message: Request) -> Result<usize> {
        let seqno = self.seqno();
        let mut tx = self.sender.lock().unwrap();
        tx.send(MetaRequest {
            seqno,
            body: message,
        })?;
        Ok(seqno)
    }

    async fn recv(&self, seqno: usize) -> Result<Response> {
        self.synchronizer.get(seqno).await
    }

    async fn fullsend(&self, message: Request) -> Result<Response> {
        let seqno = self.send(message)?;
        self.recv(seqno).await
    }

    fn send_and_ignore(&self, message: Request) -> Result<()> {
        let seqno = self.send(message)?;
        self.synchronizer.ignore(seqno);
        Ok(())
    }

    pub async fn null<'client>(&'client self) -> Result<Handle<'client, Null>> {
        let Response::Handle(index) = self.fullsend(Request::CreateNull).await? else {
            unreachable!();
        };
        Ok(Handle {
            index,
            client: self,
            _phantom: PhantomData,
        })
    }

    pub async fn word<'client>(&'client self, value: u64) -> Result<Handle<'client, Word>> {
        let Response::Handle(index) = self.fullsend(Request::CreateWord { value }).await? else {
            unreachable!();
        };
        Ok(Handle {
            index,
            client: self,
            _phantom: PhantomData,
        })
    }

    pub async fn blob<'client, T: AsRef<[u8]>>(
        &'client self,
        data: T,
    ) -> Result<Handle<'client, Blob>> {
        let data = data.as_ref();
        let mut blob = Box::new_uninit_slice_in(data.len(), self.allocator);
        blob.write_copy_of_slice(data);
        let blob = unsafe { blob.assume_init() };
        let blob = Arc::from(blob);
        let ptr: *const [u8] = Arc::into_raw(blob);
        let (ptr, len) = ptr.to_raw_parts();
        let ptr = self.allocator.to_offset(ptr);
        let Response::Handle(index) = self.fullsend(Request::CreateBlob { ptr, len }).await? else {
            unreachable!();
        };
        Ok(Handle {
            index,
            client: self,
            _phantom: PhantomData,
        })
    }
}

pub struct Null;
pub struct Word;
pub struct Blob;
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
    pub async fn read(&self) -> Result<u64> {
        let Response::Word(x) = self
            .client
            .fullsend(Request::Read { src: self.index })
            .await?
        else {
            panic!();
        };
        Ok(x)
    }
}

impl<'client> Handle<'client, Blob> {
    pub async fn create_thunk(self) -> Result<Handle<'client, Thunk>> {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::CreateThunk { src: self.index })
            .await?
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        Ok(new)
    }
}

impl<'client> Handle<'client, Thunk> {
    pub async fn run(self) -> Result<Handle<'client, Opaque>> {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::Run { src: self.index })
            .await?
        else {
            unreachable!();
        };
        let new = Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        };
        core::mem::forget(self);
        Ok(new)
    }
}

impl<T> Handle<'_, T> {
    pub async fn duplicate(&self) -> Result<Self> {
        let Response::Handle(index) = self
            .client
            .fullsend(Request::Clone { src: self.index })
            .await?
        else {
            unreachable!();
        };
        Ok(Handle {
            client: self.client,
            index,
            _phantom: PhantomData,
        })
    }
}

impl<T> Drop for Handle<'_, T> {
    fn drop(&mut self) {
        let _ = self
            .client
            .send_and_ignore(Request::Drop { src: self.index });
    }
}
