use common::util::rwlock::RwLock;
use ninep::*;
use std::sync::Arc;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt};
use tokio_vsock::{OwnedReadHalf, OwnedWriteHalf, VsockStream};
use vfs::{ErrorKind, File, SeekFrom};

#[derive(Clone)]
pub struct Vsock {
    read: Arc<RwLock<OwnedReadHalf>>,
    write: Arc<RwLock<OwnedWriteHalf>>,
}

impl Vsock {
    pub fn new(stream: VsockStream) -> Self {
        let (read, write) = stream.into_split();
        Self {
            read: Arc::new(RwLock::new(read)),
            write: Arc::new(RwLock::new(write)),
        }
    }
}

impl File for Vsock {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        let mut sock = self.read.lock();
        sock.read(bytes).await.map_err(|_| ErrorKind::Other.into())
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let mut sock = self.write.lock();
        sock.write(bytes).await.map_err(|_| ErrorKind::Other.into())
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
