use crate::prelude::*;
use common::pipe::Pipe as RawPipe;
pub use common::pipe::{Result as PipeResult};
use crate::kthread;
use core::cell::OnceCell;
use common::protocol::*;

pub static HOST: KMutex<OnceCell<Host>> = KMutex::new(OnceCell::new());

#[derive(Debug)]
pub struct HostPipe {
    inner: RawPipe,
}

impl HostPipe {
    pub fn new(pipe: RawPipe) -> Self {
        Self {
            inner: pipe
        }
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> PipeResult<usize> {
        while !self.inner.can_read() {
            // kthread::wfi();
            kthread::yield_now();
        }
        self.inner.read(bytes)
    }

    pub fn read_exact(&mut self, mut bytes: &mut [u8]) -> PipeResult<()> {
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

    pub fn write(&mut self, bytes: &[u8]) -> PipeResult<usize> {
        while !self.inner.can_write() {
            // kthread::wfi();
            kthread::yield_now();
        }
        let n = self.inner.write(bytes);
        // unsafe {
        //     crate::io::hypercall0(crate::hypercall::NOTIFY_READ);
        // }
        n
    }

    pub fn write_exact(&mut self, mut bytes: &[u8]) -> PipeResult<()> {
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
pub struct Host {
    pipe: HostPipe,
}

impl Host {
    pub fn new(pipe: HostPipe) -> Self {
        Self { pipe }
    }

    pub fn request(&mut self, request: &Request) -> Result<Response, Error> {
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
