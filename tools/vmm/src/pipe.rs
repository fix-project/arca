use crate::doorbell::{HostToVMDoorBell, VMToHostDoorBellWaiter};
use crate::session::Session;
pub use common::pipe::{Error, Result};
use common::pipe::{Pipe as RawPipe, ReadHalf, WriteHalf};
use std::{marker::PhantomData, os::fd::AsRawFd, sync::Arc};

#[derive(Debug)]
pub struct GuestReader {
    inner: ReadHalf<HostToVMDoorBell>,
    available: VMToHostDoorBellWaiter,
    session: Option<Arc<Session>>,
}

#[derive(Debug)]
pub struct GuestWriter {
    inner: WriteHalf<HostToVMDoorBell>,
    available: VMToHostDoorBellWaiter,
    session: Option<Arc<Session>>,
}

#[derive(Debug)]
pub struct GuestPipe {
    reader: GuestReader,
    writer: GuestWriter,
}

fn check(session: &Option<Arc<Session>>) -> Result<()> {
    if session.as_ref().is_some_and(|s| s.stopped()) {
        Err(Error::Closed)
    } else {
        Ok(())
    }
}

fn wait(session: &Option<Arc<Session>>, waiter: &VMToHostDoorBellWaiter) -> Result<()> {
    if let Some(session) = session {
        session
            .wait(waiter.fd.as_raw_fd(), libc::POLLIN)
            .map_err(|_| Error::Closed)?;
    }
    waiter.wait();
    check(session)
}

impl GuestReader {
    pub fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        check(&self.session)?;
        if bytes.is_empty() {
            return Ok(0);
        }
        while !self.inner.can_read() {
            wait(&self.session, &self.available)?;
        }
        self.inner.read(bytes)
    }

    pub fn read_exact(&mut self, mut bytes: &mut [u8]) -> Result<()> {
        while !bytes.is_empty() {
            check(&self.session)?;
            while !self.inner.can_read() {
                self.inner.flush();
                wait(&self.session, &self.available)?;
            }
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

    pub fn recv<R: for<'a> serde::Deserialize<'a>>(&mut self) -> Result<R> {
        let mut length = [0; 8];
        self.read_exact(&mut length)?;
        let mut bytes = vec![0; usize::from_le_bytes(length)];
        self.read_exact(&mut bytes)?;
        Ok(postcard::from_bytes(&bytes).unwrap())
    }
}

impl GuestWriter {
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        check(&self.session)?;
        if bytes.is_empty() {
            return Ok(0);
        }
        while !self.inner.can_write() {
            wait(&self.session, &self.available)?;
        }
        self.inner.write(bytes)
    }

    pub fn write_exact(&mut self, mut bytes: &[u8]) -> Result<()> {
        while !bytes.is_empty() {
            check(&self.session)?;
            while !self.inner.can_write() {
                self.inner.flush();
                wait(&self.session, &self.available)?;
            }
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

    pub fn send(&mut self, message: &impl serde::Serialize) -> Result<()> {
        let bytes = postcard::to_allocvec(message).unwrap();
        self.write_exact(&bytes.len().to_le_bytes())?;
        self.write_exact(&bytes)
    }
}

impl GuestPipe {
    pub fn new(
        pipe: RawPipe<HostToVMDoorBell>,
        rx_avail: VMToHostDoorBellWaiter,
        tx_avail: VMToHostDoorBellWaiter,
    ) -> Self {
        let (reader, writer) = pipe.split();
        Self {
            reader: GuestReader {
                inner: reader,
                available: rx_avail,
                session: None,
            },
            writer: GuestWriter {
                inner: writer,
                available: tx_avail,
                session: None,
            },
        }
    }

    pub fn with_session(mut self, session: Option<Arc<Session>>) -> Self {
        self.reader.session = session.clone();
        self.writer.session = session;
        self
    }

    pub fn split(self) -> (GuestReader, GuestWriter) {
        (self.reader, self.writer)
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        self.reader.read(bytes)
    }

    pub fn read_exact(&mut self, bytes: &mut [u8]) -> Result<()> {
        self.reader.read_exact(bytes)
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        self.writer.write(bytes)
    }

    pub fn write_exact(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_exact(bytes)
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
        Self {
            pipe,
            _send: PhantomData,
            _recv: PhantomData,
        }
    }

    pub fn recv(&mut self) -> Result<R> {
        self.pipe.reader.recv()
    }

    pub fn send(&mut self, message: &S) -> Result<()> {
        self.pipe.writer.send(message)
    }
}

pub type FilePipe = TypedPipe<common::protocol::file::Response, common::protocol::file::Request>;
pub type ListenerPipe =
    TypedPipe<common::protocol::listener::Response, common::protocol::listener::Request>;
pub type StreamPipe =
    TypedPipe<common::protocol::stream::Response, common::protocol::stream::Request>;
