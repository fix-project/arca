use core::pin::Pin;
use core::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};

use async_lock::RwLock;
use common::util::router::Router;

pub mod dir;
pub mod file;

pub use dir::*;
pub use file::*;

use super::*;
use vfs::Error;
use vfs::Result;

pub(crate) type SpawnFn<'a> = dyn Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + 'a + Send + Sync;

pub struct Client {
    conn: Arc<Connection>,
    spawn: Arc<SpawnFn<'static>>,
}

pub struct Connection {
    inbound: Router<RMessage>,
    write: RwLock<Box<dyn File>>,
    next_tag: AtomicU16,
    next_fid: AtomicU32,
    msize: AtomicUsize,
}

macro_rules! send {
    ($conn:expr; $rx:tt <- $name:ident $tx:tt) => {
        let RMessage::$name $rx = $conn
            .send(TMessage::$name $tx)
            .await??
        else {
            return Err(Error::from(ErrorKind::Other));
        };
    };
}
pub(crate) use send;

impl Connection {
    fn tag(&self) -> Tag {
        Tag(self.next_tag.fetch_add(1, Ordering::Relaxed))
    }

    fn fid(&self) -> Fid {
        Fid(self.next_fid.fetch_add(1, Ordering::Relaxed))
    }

    fn msize(&self) -> usize {
        self.msize.load(Ordering::Relaxed)
    }

    async fn send(&self, message: TMessage) -> Result<RMessage> {
        let tag = message.tag();
        log::debug!("-> {message:?}");
        let msg = wire::to_bytes_with_len(message)?;
        let mut f = self.write.write().await;
        f.write(&msg).await?;
        let result = self.inbound.recv(tag.0 as u64).await;
        log::debug!("<- {result:?}");
        Ok(result)
    }
}

impl Client {
    pub async fn new(
        connection: impl File + 'static,
        spawn: impl Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + 'static + Send + Sync,
    ) -> Result<Client> {
        let inbound = Router::new();
        let tx = inbound.clone();
        let connection = connection.boxed();
        let read = connection.dup().await?;
        spawn(Box::pin(async move {
            Self::bg_task(tx, read).await.expect("connection closed");
        }));
        let conn = Arc::new(Connection {
            inbound,
            write: RwLock::new(connection),
            next_tag: AtomicU16::new(0),
            next_fid: AtomicU32::new(0),
            msize: AtomicUsize::new(1024),
        });
        let max_msize = 1024 * 64;
        send!(conn; {msize, version, ..} <- Version {tag: conn.tag(), msize: max_msize, version: "9P2000".into()});
        if msize > max_msize {
            return Err(Error::from(ErrorKind::Other));
        }
        if version != "9P2000" {
            log::error!("server only supports {version}");
            return Err(Error::from(ErrorKind::Other));
        }
        conn.msize.store(msize as usize, Ordering::Relaxed);
        Ok(Client {
            conn,
            spawn: Arc::new(spawn),
        })
    }

    async fn bg_task(tx: Router<RMessage>, mut read: Box<dyn File>) -> Result<()> {
        loop {
            let mut size = [0u8; 4];
            read.read_exact(&mut size).await?;
            let size = u32::from_le_bytes(size);
            let mut buf = vec![0; size as usize - 4];
            read.read_exact(&mut buf).await?;
            let rmsg: RMessage = wire::from_bytes(&buf)?;
            tx.send(rmsg.tag().0 as u64, rmsg);
        }
    }

    pub async fn auth(&self, uname: &str, aname: &str) -> Result<File9P> {
        let tag = self.conn.tag();
        let afid = self.conn.fid();
        send!(self.conn; {aqid, ..} <- Auth {tag, afid, uname: uname.to_owned(), aname: aname.to_owned()});
        Ok(File9P {
            conn: self.conn.clone(),
            fid: afid,
            qid: aqid,
            cursor: 0,
            spawn: self.spawn.clone(),
        })
    }

    pub async fn attach(&self, auth: Option<File9P>, uname: &str, aname: &str) -> Result<Dir9P> {
        let tag = self.conn.tag();
        let fid = self.conn.fid();
        let afid = auth.map(|x| x.fid).unwrap_or(Fid(!0));
        send!(self.conn; {qid, ..} <- Attach {tag, fid, afid, uname: uname.to_owned(), aname: aname.to_owned(), n_uname: None});
        Ok(Dir9P {
            conn: self.conn.clone(),
            fid,
            qid,
            spawn: self.spawn.clone(),
        })
    }
}
