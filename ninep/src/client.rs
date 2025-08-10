use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU16, AtomicU32, Ordering},
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
}

impl Connection {
    fn tag(&self) -> Tag {
        Tag(self.next_tag.fetch_add(1, Ordering::Relaxed))
    }

    fn fid(&self) -> Fid {
        Fid(self.next_fid.fetch_add(1, Ordering::Relaxed))
    }

    async fn send(&self, message: TMessage) -> Result<RMessage> {
        let mut f = self.inbox.conn.lock();
        let tag = message.tag();
        let msg = wire::to_bytes_with_len(message)?;
        f.write(&msg).await?;
        SpinLock::unlock(f);
        let result = self.inbox.read(tag).await?;
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
        });
        let msg = TMessage::Version {
            tag: conn.tag(),
            msize: 4096,
            version: "9P2000".into(),
        };
        let RMessage::Version { msize, version, .. } = conn.send(msg).await? else {
            return Err(Error::InputOutputError);
        };
        if msize != 4096 {
            return Err(Error::InputOutputError);
        }
        if version != "9P2000" {
            return Err(Error::InputOutputError);
        }
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
            connection: self.conn.clone(),
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
            })
            .await??
        else {
            return Err(Error::InputOutputError);
        };
        Ok(P9 {
            connection: self.conn.clone(),
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
        let mut buf = vec![0; 4096];
        self.sem.acquire(1).await;
        let mut storage = self.storage.write();
        if let Some(result) = storage.remove(&tag) {
            self.sem.release(1);
            return Ok(result);
        }
        // TODO: fix head-of-line blocking here
        loop {
            let n = self.conn.lock().read(&mut buf).await?;
            let rmsg: RMessage = wire::from_bytes_with_len(&buf[..n])?;
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
    connection: Arc<Connection>,
    fid: Fid,
    qid: Qid,
    _phantom: PhantomData<T>,
}
