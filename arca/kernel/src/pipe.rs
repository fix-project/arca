use crate::doorbell::VMToHostDoorBell;
use crate::kthread;
use crate::prelude::*;
use alloc::collections::BTreeMap;
pub use common::pipe::Result as PipeResult;
use common::pipe::{Pipe as RawPipe, ReadHalf, WriteHalf};
use common::protocol::control;
use common::util::{initcell::OnceLock, spinlock::SpinLock};
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};

pub static HOST: OnceLock<ControlClient> = OnceLock::new();

#[derive(Debug)]
pub struct HostReader {
    inner: ReadHalf<VMToHostDoorBell>,
}

#[derive(Debug)]
pub struct HostWriter {
    inner: WriteHalf<VMToHostDoorBell>,
}

#[derive(Debug)]
pub struct HostPipe {
    reader: HostReader,
    writer: HostWriter,
}

impl HostReader {
    pub fn read_exact(&mut self, mut bytes: &mut [u8]) -> PipeResult<()> {
        while !bytes.is_empty() {
            if !self.inner.can_read() {
                self.inner.flush();
            }
            kthread::wait_until(|| self.inner.can_read());
            match self.inner.read_deferred(bytes) {
                Ok(n) => {
                    bytes = &mut bytes[n..];
                }
                Err(e) => {
                    self.inner.flush();
                    return Err(e);
                }
            }
        }
        self.inner.flush();
        Ok(())
    }

    fn read_message(&mut self) -> PipeResult<Vec<u8>> {
        let mut length = [0; 8];
        self.read_exact(&mut length)?;
        let mut bytes = vec![0; usize::from_le_bytes(length)];
        self.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

impl HostWriter {
    pub fn write_exact(&mut self, mut bytes: &[u8]) -> PipeResult<()> {
        while !bytes.is_empty() {
            if !self.inner.can_write() {
                self.inner.flush();
            }
            kthread::wait_until(|| self.inner.can_write());
            match self.inner.write_deferred(bytes) {
                Ok(n) => {
                    bytes = &bytes[n..];
                }
                Err(e) => {
                    self.inner.flush();
                    return Err(e);
                }
            }
        }
        self.inner.flush();
        Ok(())
    }

    fn write_message(&mut self, bytes: &[u8]) -> PipeResult<()> {
        self.write_exact(&bytes.len().to_le_bytes())?;
        self.write_exact(bytes)
    }
}

impl HostPipe {
    pub fn new(pipe: RawPipe<VMToHostDoorBell>) -> Self {
        let (reader, writer) = pipe.split();
        Self {
            reader: HostReader { inner: reader },
            writer: HostWriter { inner: writer },
        }
    }

    pub fn read_exact(&mut self, bytes: &mut [u8]) -> PipeResult<()> {
        self.reader.read_exact(bytes)
    }

    pub fn write_exact(&mut self, bytes: &[u8]) -> PipeResult<()> {
        self.writer.write_exact(bytes)
    }
}

struct Waiter {
    thread: Arc<kthread::Thread>,
    response: SpinLock<Option<control::Response>>,
}

/// The control channel to the host, which may carry many requests at once. Whichever waiting
/// thread holds the reader delivers responses to the others until its own arrives.
pub struct ControlClient {
    writer: KMutex<HostWriter>,
    reader: KMutex<HostReader>,
    waiters: SpinLock<BTreeMap<u64, Arc<Waiter>>>,
    next: AtomicU64,
}

impl ControlClient {
    pub fn new(pipe: HostPipe) -> Self {
        let HostPipe { reader, writer } = pipe;
        Self {
            writer: KMutex::new(writer),
            reader: KMutex::new(reader),
            waiters: SpinLock::new(BTreeMap::new()),
            next: AtomicU64::new(0),
        }
    }

    pub fn request(&self, request: &control::Request) -> control::Response {
        self.request_serialized(request)
    }

    pub fn request_serialized(&self, request: &impl serde::Serialize) -> control::Response {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let waiter = Arc::new(Waiter {
            thread: kthread::current(),
            response: SpinLock::new(None),
        });
        self.waiters.lock().insert(id, waiter.clone());
        let bytes = postcard::to_allocvec(&control::Tagged {
            id,
            message: request,
        })
        .unwrap();
        self.writer.lock().write_message(&bytes).unwrap();
        loop {
            if let Some(response) = waiter.response.lock().take() {
                return response;
            }
            if let Some(mut reader) = self.reader.try_lock() {
                while waiter.response.lock().is_none() {
                    self.deliver(&mut reader);
                }
                drop(reader);
                let next = self.waiters.lock().values().next().cloned();
                if let Some(next) = next {
                    next.thread.unpark();
                }
                continue;
            }
            kthread::park();
        }
    }

    fn deliver(&self, reader: &mut HostReader) {
        let bytes = reader.read_message().unwrap();
        let response: control::Tagged<control::Response> = postcard::from_bytes(&bytes).unwrap();
        let waiter = self
            .waiters
            .lock()
            .remove(&response.id)
            .expect("response without a request");
        *waiter.response.lock() = Some(response.message);
        waiter.thread.unpark();
    }
}

#[derive(Debug)]
pub struct TypedPipe<S, R> {
    pipe: HostPipe,
    _send: PhantomData<S>,
    _recv: PhantomData<R>,
}

impl<S: serde::Serialize, R: for<'a> serde::Deserialize<'a>> TypedPipe<S, R> {
    pub fn new(pipe: HostPipe) -> Self {
        Self {
            pipe,
            _send: PhantomData,
            _recv: PhantomData,
        }
    }

    pub fn request(&mut self, request: &S) -> R {
        self.request_serialized(request)
    }

    pub(crate) fn request_serialized(&mut self, request: &impl serde::Serialize) -> R {
        let bytes = postcard::to_allocvec(request).unwrap();
        let length = bytes.len().to_le_bytes();
        self.pipe.write_exact(&length).unwrap();
        self.pipe.write_exact(&bytes).unwrap();
        let mut length = [0; 8];
        self.pipe.read_exact(&mut length).unwrap();
        let length = usize::from_le_bytes(length);
        let mut bytes = vec![0; length];
        self.pipe.read_exact(&mut bytes).unwrap();
        postcard::from_bytes(&bytes).unwrap()
    }
}

pub type FilePipe = TypedPipe<common::protocol::file::Request, common::protocol::file::Response>;
pub type ListenerPipe =
    TypedPipe<common::protocol::listener::Request, common::protocol::listener::Response>;
pub type StreamPipe =
    TypedPipe<common::protocol::stream::Request, common::protocol::stream::Response>;
