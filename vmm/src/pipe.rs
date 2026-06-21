use common::pipe::Pipe as RawPipe;
pub use common::pipe::{Error, Result};
use vmm_sys_util::eventfd::EventFd;

#[allow(dead_code)]
pub struct GuestPipe {
    read_fd: EventFd,
    write_fd: EventFd,
    inner: RawPipe,
}

impl GuestPipe {
    pub fn new(read_fd: EventFd, write_fd: EventFd, pipe: RawPipe) -> Self {
        Self {
            read_fd,
            write_fd,
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
