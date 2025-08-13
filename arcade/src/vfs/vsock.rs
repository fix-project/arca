use super::*;
use kernel::virtio::vsock::{SocketAddr, Stream};

#[derive(Debug, Default, Clone, Copy)]
pub struct VSockFS;

impl VSockFS {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl VirtualOpenDir for VSockFS {}

#[async_trait]
impl VirtualClosedDir for VSockFS {
    fn init(&self) -> BTreeMap<String, VClosedNode> {
        let mut map = BTreeMap::new();
        map.insert("listen".to_owned(), VClosedNode::Dir(Listen.into()));
        map.insert("connect".to_owned(), VClosedNode::Dir(Connect.into()));
        map
    }

    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenDir>> {
        Ok(Box::new(*self))
    }

    async fn mkdir(&mut self, _: &str, _: BitFlags<Perm>, _: BitFlags<Flag>) -> Result<VClosedDir> {
        Err(Error::PermissionDenied)
    }

    async fn create(
        &mut self,
        _: &str,
        _: BitFlags<Perm>,
        _: BitFlags<Flag>,
    ) -> Result<VClosedFile> {
        Err(Error::PermissionDenied)
    }
}

#[derive(Copy, Clone)]
struct Listen;

#[derive(Copy, Clone)]
struct Connect;

#[async_trait]
impl VirtualOpenDir for Listen {}

#[async_trait]
impl VirtualClosedDir for Listen {
    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenDir>> {
        Ok(Box::new(*self))
    }

    async fn mkdir(&mut self, _: &str, _: BitFlags<Perm>, _: BitFlags<Flag>) -> Result<VClosedDir> {
        Err(Error::PermissionDenied)
    }

    async fn create(
        &mut self,
        _: &str,
        _: BitFlags<Perm>,
        _: BitFlags<Flag>,
    ) -> Result<VClosedFile> {
        Err(Error::PermissionDenied)
    }
}

#[async_trait]
impl VirtualOpenDir for Connect {}

#[async_trait]
impl VirtualClosedDir for Connect {
    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenDir>> {
        Ok(Box::new(*self))
    }

    async fn mkdir(&mut self, _: &str, _: BitFlags<Perm>, _: BitFlags<Flag>) -> Result<VClosedDir> {
        Err(Error::OperationNotPermitted)
    }

    async fn create(
        &mut self,
        name: &str,
        _: BitFlags<Perm>,
        flags: BitFlags<Flag>,
    ) -> Result<VClosedFile> {
        if !flags.contains(Flag::Exclusive) {
            return Err(Error::OperationNotPermitted);
        }
        let (cid, port) = name.split_once(":").ok_or(Error::NoSuchFileOrDirectory)?;
        let cid: u64 = str::parse(cid).map_err(|_| Error::NoSuchFileOrDirectory)?;
        let port: u32 = str::parse(port).map_err(|_| Error::NoSuchFileOrDirectory)?;

        let local = SocketAddr { cid: 3, port: 0 };
        let peer = SocketAddr { cid, port };

        Ok(VClosedFile::from(ClosedConnection { local, peer }))
    }
}

struct ClosedConnection {
    local: SocketAddr,
    peer: SocketAddr,
}

#[async_trait]
impl VirtualClosedFile for ClosedConnection {
    async fn open(&self, _: Access) -> Result<Box<dyn VirtualOpenFile>> {
        let stream = Stream::connect(self.local, self.peer)
            .await
            .map_err(|_| Error::Message("unable to connect to socket".to_owned()))?;
        Ok(Box::new(OpenConnection {
            local: self.local,
            peer: self.peer,
            stream,
        }))
    }
}

struct OpenConnection {
    local: SocketAddr,
    peer: SocketAddr,
    stream: Stream,
}

#[async_trait]
impl VirtualOpenFile for OpenConnection {
    async fn read(&self, _: usize, buf: &mut [u8]) -> Result<usize> {
        let mut read = 0;
        while read < buf.len() {
            read += self
                .stream
                .recv(&mut buf[read..])
                .await
                .map_err(|_| Error::InputOutputError)?;
        }
        Ok(read)
    }

    async fn write(&mut self, _: usize, buf: &[u8]) -> Result<usize> {
        Ok(self
            .stream
            .send(buf)
            .await
            .map_err(|_| Error::InputOutputError)?)
    }
}
