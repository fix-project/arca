use common::pipe::Pipe as RawPipe;
pub use common::pipe::{Error, Result};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct GuestPipe {
    inner: RawPipe,
}

impl GuestPipe {
    pub fn new(pipe: RawPipe) -> Self {
        Self {
            inner: pipe
        }
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        while !self.inner.can_read() {
            // self.read_fd.read().unwrap();
            std::thread::yield_now();
        }
        self.inner.read(bytes)
    }

    pub fn read_exact(&mut self, mut bytes: &mut [u8]) -> Result<()> {
        while !bytes.is_empty() {
            match self.read(bytes) {
                Ok(n) => {
                    bytes = &mut bytes[n..];
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        while !self.inner.can_write() {
            // self.write_fd.read().unwrap();
            std::thread::yield_now();
        }
        self.inner.write(bytes)
    }

    pub fn write_exact(&mut self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            match self.write(bytes) {
                Ok(n) => {
                    bytes = &bytes[n..];
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct TypedPipe<S, R> {
    pipe: GuestPipe,
    _send: PhantomData<S>,
    _recv: PhantomData<R>,
}

impl<S: serde::Serialize, R: for<'a> serde::Deserialize<'a>> TypedPipe<S, R> {
    pub fn new(pipe: GuestPipe) -> Self {
        Self { pipe, _send: PhantomData, _recv: PhantomData }
    }

    pub fn recv(&mut self) -> R {
        let mut length = [0; 8];
        self.pipe.read_exact(&mut length).unwrap();
        let length = usize::from_le_bytes(length);
        let mut bytes = vec![0; length];
        self.pipe.read_exact(&mut bytes).unwrap();
        postcard::from_bytes(&bytes).unwrap()
    }

    pub fn send(&mut self, request: &S) {
        let bytes = postcard::to_allocvec(request).unwrap();
        let length = bytes.len().to_le_bytes();
        self.pipe.write_exact(&length).unwrap();
        self.pipe.write_exact(&bytes).unwrap();
    }
}

pub type ControlPipe = TypedPipe<common::protocol::control::Response, common::protocol::control::Request>;
pub type FilePipe = TypedPipe<common::protocol::file::Response, common::protocol::file::Request>;
pub type ListenerPipe = TypedPipe<common::protocol::listener::Response, common::protocol::listener::Request>;
pub type StreamPipe = TypedPipe<common::protocol::stream::Response, common::protocol::stream::Request>;
