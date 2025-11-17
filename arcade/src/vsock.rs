use super::*;
use chumsky::Parser;
use common::util::descriptors::Descriptors;
use futures::{StreamExt, stream::BoxStream};
use kernel::virtio::vsock::{SocketAddr, Stream, StreamListener};

#[derive(Clone, Default)]
pub struct VSockFS {
    cid: u64,
    conns: Arc<SpinLock<Descriptors<Arc<SpinLock<Connection>>>>>,
}

impl VSockFS {
    pub fn new(cid: u64) -> Self {
        VSockFS {
            cid,
            conns: Arc::new(SpinLock::new(Descriptors::default())),
        }
    }
}

#[derive(Clone)]
struct Connection {
    cid: u64,
    index: usize,
    state: State,
    conns: Arc<SpinLock<Descriptors<Arc<SpinLock<Connection>>>>>,
}

#[derive(Clone)]
enum State {
    Idle,
    Connected(Arc<Stream>),
    Listening(Arc<StreamListener>),
}

#[derive(Clone)]
struct ConnDir {
    open: Open,
    conn: Arc<SpinLock<Connection>>,
}

#[derive(Clone)]
struct Control {
    open: Open,
    conn: Arc<SpinLock<Connection>>,
}

#[derive(Clone)]
struct Data {
    open: Open,
    conn: Arc<SpinLock<Connection>>,
}

impl Dir for VSockFS {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        if name.is_empty() {
            return Ok(self.dup().await?.boxed().into());
        }
        let mut conns = self.conns.lock();
        if name == "clone" {
            let conn = Arc::new(SpinLock::new(Connection {
                cid: self.cid,
                index: 0,
                state: State::Idle,
                conns: self.conns.clone(),
            }));
            let index = conns.insert(conn.clone());
            conn.lock().index = index;
            return Ok(Object::File(Control { open, conn }.boxed()));
        }
        let i: usize = str::parse(name).map_err(|_| ErrorKind::NotFound)?;
        let conn = conns.get(i).ok_or(ErrorKind::NotFound)?.clone();
        Ok(Object::Dir(ConnDir { open, conn }.boxed()))
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        let conns = self.conns.lock();
        let mut v = vec![DirEnt {
            name: "clone".to_string(),
            dir: false,
        }];
        for (k, _) in conns.iter() {
            v.push(DirEnt {
                name: k.to_string(),
                dir: true,
            });
        }
        Ok(futures::stream::iter(v.into_iter().map(Ok)).boxed())
    }

    async fn create(&self, _: &str, _: Create, _: Open) -> Result<Object> {
        Err(ErrorKind::PermissionDenied.into())
    }

    async fn remove(&self, _: &str) -> Result<()> {
        Err(ErrorKind::PermissionDenied.into())
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

impl Dir for ConnDir {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        if !open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        Ok(Object::File(match name {
            "ctl" => Control {
                open,
                conn: self.conn.clone(),
            }
            .boxed(),
            "data" => Data {
                open,
                conn: self.conn.clone(),
            }
            .boxed(),
            "listen" => {
                let State::Listening(listener) = self.conn.lock().state.clone() else {
                    return Err(ErrorKind::ResourceBusy.into());
                };
                let stream = listener
                    .accept()
                    .await
                    .map_err(|_| ErrorKind::ResourceBusy)?;
                let conn = self.conn.lock();
                let new = Arc::new(SpinLock::new(Connection {
                    cid: conn.cid,
                    index: 0,
                    state: State::Connected(stream.into()),
                    conns: conn.conns.clone(),
                }));
                let mut conns = conn.conns.lock();
                let index = conns.insert(new.clone());
                new.lock().index = index;
                Control { open, conn: new }
            }
            .boxed(),
            _ => return Err(ErrorKind::NotFound.into()),
        }))
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        Ok(futures::stream::iter(
            [
                DirEnt {
                    name: "ctl".to_string(),
                    dir: false,
                },
                DirEnt {
                    name: "data".to_string(),
                    dir: false,
                },
                DirEnt {
                    name: "listen".to_string(),
                    dir: false,
                },
            ]
            .map(Ok),
        )
        .boxed())
    }

    async fn create(&self, _: &str, _: Create, _: Open) -> Result<Object> {
        Err(ErrorKind::PermissionDenied.into())
    }

    async fn remove(&self, _: &str) -> Result<()> {
        Err(ErrorKind::PermissionDenied.into())
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

enum Command {
    Connect(SocketAddr),
    Announce(u32),
    Hangup,
}

fn command<'a>() -> impl Parser<'a, &'a str, Command> {
    use chumsky::prelude::*;
    let uint32 = text::int(10).from_str::<u32>().unwrapped();
    let uint64 = text::int(10).from_str::<u64>().unwrapped();
    let sockaddr = uint64
        .then_ignore(just(":").ignored())
        .then(uint32)
        .map(|(cid, port)| SocketAddr { cid, port });
    choice((
        just("connect")
            .padded()
            .ignore_then(sockaddr)
            .then_ignore(text::newline())
            .map(Command::Connect),
        just("announce")
            .padded()
            .ignore_then(uint32)
            .then_ignore(text::newline())
            .map(Command::Announce),
        just("hangup")
            .then_ignore(text::newline())
            .map(|_| Command::Hangup),
    ))
}

impl File for Control {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let conn = self.conn.lock();
        let data = conn.index.to_string() + "\n";
        let n = core::cmp::min(data.len(), bytes.len());
        bytes[..n].copy_from_slice(&data.as_bytes()[..n]);
        Ok(n)
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let msg = str::from_utf8(bytes).map_err(|_| ErrorKind::InvalidInput)?;
        let result = command()
            .parse(msg)
            .into_result()
            .map_err(|_| ErrorKind::InvalidInput)?;
        match result {
            Command::Connect(dst) => {
                let mut conn = self.conn.lock();
                let State::Idle = conn.state else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let stream = Stream::connect(conn.cid, dst)
                    .await
                    .map_err(|_| ErrorKind::ResourceBusy)?;
                conn.state = State::Connected(Arc::new(stream));
            }
            Command::Announce(port) => {
                let mut conn = self.conn.lock();
                let State::Idle = conn.state else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let listener = StreamListener::bind(conn.cid, port)
                    .await
                    .map_err(|_| ErrorKind::ResourceBusy)?;
                conn.state = State::Listening(Arc::new(listener));
            }
            Command::Hangup => {
                let mut conn = self.conn.lock();
                match conn.state {
                    State::Idle => return Err(ErrorKind::InvalidInput.into()),
                    _ => conn.state = State::Idle,
                }
            }
        }
        Ok(bytes.len())
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

impl File for Data {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let conn = self.conn.lock().clone();
        let State::Connected(stream) = conn.state else {
            return Err(ErrorKind::ResourceBusy.into());
        };
        stream
            .recv(bytes)
            .await
            .map_err(|_| ErrorKind::Other.into())
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let conn = self.conn.lock().clone();
        let State::Connected(stream) = conn.state else {
            return Err(ErrorKind::ResourceBusy.into());
        };
        stream
            .send(bytes)
            .await
            .map_err(|_| ErrorKind::Other.into())
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
