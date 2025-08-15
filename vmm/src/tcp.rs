use std::net::SocketAddr;

use super::*;
use chumsky::error::Rich;
use chumsky::{extra, Parser};
use common::util::{descriptors::Descriptors, spinlock::SpinLock};
use futures::{stream::BoxStream, StreamExt};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use vfs::*;

#[derive(Clone, Default)]
pub struct TcpFS {
    conns: Arc<SpinLock<Descriptors<Arc<SpinLock<Connection>>>>>,
}

#[derive(Clone)]
struct Connection {
    index: usize,
    state: State,
    conns: Arc<SpinLock<Descriptors<Arc<SpinLock<Connection>>>>>,
}

#[derive(Clone)]
enum State {
    Idle,
    Connected(Arc<SpinLock<TcpStream>>),
    Listening(Arc<SpinLock<TcpListener>>),
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

impl Dir for TcpFS {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        let mut conns = self.conns.lock();
        if name == "clone" {
            let conn = Arc::new(SpinLock::new(Connection {
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
        return Err(ErrorKind::PermissionDenied.into());
    }

    async fn remove(&self, _: &str) -> Result<()> {
        return Err(ErrorKind::PermissionDenied.into());
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

impl Dir for ConnDir {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        if !self.open.contains(Open::Read) {
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
                let conn = self.conn.lock();
                let State::Listening(listener) = &conn.state else {
                    log::error!("not listening?");
                    return Err(ErrorKind::ResourceBusy.into());
                };
                let listener = listener.lock();
                let (stream, _) = listener
                    .accept()
                    .await
                    .map_err(|_| ErrorKind::ResourceBusy)?;
                let new = Arc::new(SpinLock::new(Connection {
                    index: 0,
                    state: State::Connected(Arc::new(SpinLock::new(stream))),
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
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
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
        return Err(ErrorKind::PermissionDenied.into());
    }

    async fn remove(&self, _: &str) -> Result<()> {
        return Err(ErrorKind::PermissionDenied.into());
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

#[derive(Debug)]
enum Command {
    Connect(SocketAddr),
    Announce(SocketAddr),
    Hangup,
}

fn command<'a>() -> impl Parser<'a, &'a str, Command, extra::Err<Rich<'a, char>>> {
    use chumsky::prelude::*;
    let sockaddr = any()
        .filter(|x: &char| !x.is_whitespace())
        .repeated()
        .collect()
        .try_map(|s: String, span| {
            s.parse::<SocketAddr>()
                .map_err(|_| Rich::custom(span, "failed to parse socket address"))
        });
    choice((
        just("connect")
            .padded()
            .ignore_then(sockaddr)
            .map(Command::Connect)
            .then_ignore(text::newline()),
        just("announce")
            .padded()
            .ignore_then(sockaddr)
            .map(Command::Announce)
            .then_ignore(text::newline()),
        just("hangup")
            .map(|_| Command::Hangup)
            .then_ignore(text::newline()),
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
        let result = command().parse(msg).into_result();
        let output = result.map_err(|_| ErrorKind::InvalidInput)?;
        match output {
            Command::Connect(dst) => {
                let mut conn = self.conn.lock();
                let State::Idle = conn.state else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let stream = TcpStream::connect(dst)
                    .await
                    .map_err(|_| ErrorKind::ResourceBusy)?;
                conn.state = State::Connected(Arc::new(SpinLock::new(stream)));
            }
            Command::Announce(addr) => {
                let mut conn = self.conn.lock();
                let State::Idle = conn.state else {
                    return Err(ErrorKind::InvalidInput.into());
                };
                let listener = TcpListener::bind(addr)
                    .await
                    .map_err(|_| ErrorKind::ResourceBusy)?;
                conn.state = State::Listening(Arc::new(SpinLock::new(listener)));
            }
            Command::Hangup => {
                let mut conn = self.conn.lock();
                match conn.state {
                    State::Idle => return Err(ErrorKind::InvalidInput.into()),
                    _ => conn.state = State::Idle,
                }
                let mut conns = conn.conns.lock();
                conns.remove(conn.index);
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
        let conn = self.conn.lock();
        let State::Connected(stream) = &conn.state else {
            return Err(ErrorKind::ResourceBusy.into());
        };
        let mut stream = stream.lock();
        stream
            .read(bytes)
            .await
            .map_err(|_| ErrorKind::Other.into())
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let conn = self.conn.lock();
        let State::Connected(stream) = &conn.state else {
            return Err(ErrorKind::ResourceBusy.into());
        };
        let mut stream = stream.lock();
        stream
            .write(bytes)
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
