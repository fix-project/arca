use core::{
    marker::PhantomData,
    sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicUsize, Ordering},
    u32,
};

use alloc::collections::btree_map::BTreeMap;
use common::util::{
    channel::ChannelClosed, rwlock::RwLock, semaphore::Semaphore, spinlock::SpinLock,
};

pub mod dir;
pub mod file;
pub mod node;

pub use super::*;

pub struct Client {
    conn: Arc<Connection>,
}

struct Connection {
    inbox: Demultiplexer,
    next_tag: AtomicU16,
    next_fid: AtomicU32,
    msize: AtomicUsize,
    linux: AtomicBool,
}

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

    fn linux(&self) -> bool {
        self.linux.load(Ordering::Relaxed)
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
    pub async fn new(connection: OpenFile) -> Result<Client> {
        let inbox = Demultiplexer::new(connection).await?;
        let conn = Arc::new(Connection {
            inbox,
            next_tag: AtomicU16::new(0),
            next_fid: AtomicU32::new(0),
            msize: AtomicUsize::new(1024),
            linux: AtomicBool::new(false),
        });
        let max_msize = 1024 * 64;
        let msg = TMessage::Version {
            tag: conn.tag(),
            msize: max_msize,
            version: "9P2000.L".into(),
        };
        let RMessage::Version { msize, version, .. } = conn.send(msg).await? else {
            return Err(Error::InputOutputError);
        };
        if msize > max_msize {
            return Err(Error::InputOutputError);
        }
        if version == "9P2000" {
            conn.linux.store(false, Ordering::Relaxed);
        } else if version == "9P2000.L" {
            conn.linux.store(true, Ordering::Relaxed);
        } else {
            log::error!("server only supports {version}");
            return Err(Error::InputOutputError);
        }
        conn.msize.store(msize as usize, Ordering::Relaxed);
        Ok(Client { conn })
    }

    pub async fn auth(&self, uname: &str, aname: &str) -> Result<OpenFile> {
        let tag = self.conn.tag();
        let afid = self.conn.fid();
        let RMessage::Auth { aqid, .. } = self
            .conn
            .send(TMessage::Auth {
                tag,
                afid,
                uname: uname.to_owned(),
                aname: aname.to_owned(),
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(P9 {
            conn: self.conn.clone(),
            fid: afid,
            qid: aqid,
            _phantom: PhantomData,
        }
        .into())
    }

    pub async fn attach(
        &self,
        auth: Option<P9<OpenFile>>,
        uname: &str,
        aname: &str,
    ) -> Result<P9<ClosedDir>> {
        let tag = self.conn.tag();
        let fid = self.conn.fid();
        let afid = auth.map(|x| x.fid).unwrap_or(Fid(!0));
        let RMessage::Attach { qid, .. } = self
            .conn
            .send(TMessage::Attach {
                tag,
                fid,
                afid,
                uname: uname.to_owned(),
                aname: aname.to_owned(),
                n_uname: if self.conn.linux() { Some(!0) } else { None },
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(P9 {
            conn: self.conn.clone(),
            fid,
            qid,
            _phantom: PhantomData,
        })
    }
}

impl From<ChannelClosed> for Error {
    fn from(_: ChannelClosed) -> Self {
        Error::Message("connection to server closed".to_owned())
    }
}

struct Demultiplexer {
    conn: SpinLock<OpenFile>,
    sem: Semaphore,
    storage: Arc<RwLock<BTreeMap<Tag, RMessage>>>,
}

impl Demultiplexer {
    pub async fn new(conn: OpenFile) -> Result<Demultiplexer> {
        let conn = SpinLock::new(conn);
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

pub struct P9<T: NodeType> {
    conn: Arc<Connection>,
    fid: Fid,
    qid: Qid,
    _phantom: PhantomData<T>,
}
