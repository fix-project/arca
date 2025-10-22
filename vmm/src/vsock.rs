use ninep::*;
use smol::{
    io::{AsyncReadExt as _, AsyncWriteExt, ReadHalf, WriteHalf},
    lock::Mutex,
    Async,
};
use std::sync::Arc;
use vfs::{ErrorKind, File, SeekFrom};
use vsock::VsockStream;

#[derive(Clone)]
pub struct Vsock {
    read: Arc<Mutex<ReadHalf<Async<&'static VsockStream>>>>,
    write: Arc<Mutex<WriteHalf<Async<&'static VsockStream>>>>,
}

impl Vsock {
    pub fn new(stream: VsockStream) -> Self {
        stream.set_nonblocking(true).unwrap();
        let stream = Box::leak(Box::new(stream));
        let stream = Async::new(&*stream).unwrap();
        let (read, write) = smol::io::split(stream);
        Self {
            read: Arc::new(Mutex::new(read)),
            write: Arc::new(Mutex::new(write)),
        }
    }
}

impl File for Vsock {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        let x = self
            .read
            .lock()
            .await
            .read(bytes)
            .await
            .map_err(|_| ErrorKind::Other.into());
        x
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let x = self
            .write
            .lock()
            .await
            .write(bytes)
            .await
            .map_err(|_| ErrorKind::Other.into());
        x
    }

    async fn seek(&mut self, _: SeekFrom) -> Result<usize> {
        Err(ErrorKind::NotSeekable.into())
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }

    fn seekable(&self) -> bool {
        false
    }
}
