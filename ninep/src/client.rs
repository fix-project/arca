use core::sync::atomic::{AtomicU16, AtomicU32, AtomicUsize, Ordering};

use alloc::collections::btree_map::BTreeMap;
use common::util::{rwlock::RwLock, semaphore::Semaphore, spinlock::SpinLock};

pub mod dir;
pub mod file;

pub use dir::*;
pub use file::*;

use super::*;
use vfs::Error;
use vfs::Result;

pub struct Client {
    conn: Arc<Connection>,
}

pub struct Connection {
    inbox: Demultiplexer,
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
        let mut f = self.inbox.conn.lock();
        let tag = message.tag();
        log::debug!("-> {message:?}");
        let msg = wire::to_bytes_with_len(message)?;
        f.write(&msg).await?;
        SpinLock::unlock(f);
        let result = self.inbox.read(tag).await?;
        log::debug!("<- {result:?}");
        Ok(result)
    }
}

impl Client {
    pub async fn new(connection: impl File + 'static) -> Result<Client> {
        let inbox = Demultiplexer::new(connection).await?;
        let conn = Arc::new(Connection {
            inbox,
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
        Ok(Client { conn })
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
        })
    }
}

struct Demultiplexer {
    conn: SpinLock<Box<dyn File>>,
    sem: Semaphore,
    storage: Arc<RwLock<BTreeMap<Tag, RMessage>>>,
}

impl Demultiplexer {
    pub async fn new(conn: impl File) -> Result<Demultiplexer> {
        let conn = SpinLock::new(conn.boxed());
        let sem = Semaphore::new(1);
        let storage = Arc::default();
        Ok(Demultiplexer { conn, sem, storage })
    }

    pub async fn read(&self, tag: Tag) -> Result<RMessage> {
        self.sem.acquire(1).await;
        let mut storage = self.storage.write();
        if let Some(result) = storage.remove(&tag) {
            self.sem.release(1);
            return Ok(result);
        }
        // TODO: fix head-of-line blocking here
        loop {
            let mut size = [0u8; 4];
            self.conn.lock().read(&mut size).await?;
            let size = u32::from_le_bytes(size);
            let mut buf = vec![0; size as usize - 4];
            let n = self.conn.lock().read(&mut buf).await?;
            let rmsg: RMessage = wire::from_bytes(&buf[..n])?;
            if rmsg.tag() == tag {
                self.sem.release(1);
                return Ok(rmsg);
            } else {
                storage.insert(rmsg.tag(), rmsg);
            }
        }
    }
}
