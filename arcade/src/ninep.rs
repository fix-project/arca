pub mod wire;
use core::task::Waker;

use alloc::collections::btree_map::BTreeMap;
use kernel::prelude::*;

use crate::{
    fs::{self, Backend, Error, Fid, Object, Result, Tag},
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub struct Qid {
    pub qtype: u8,
    pub version: u32,
    pub path: u64,
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum TMessage {
    #[serde(rename = "100")]
    Version {
        tag: Tag,
        msize: u32,
        version: String,
    },
    #[serde(rename = "102")]
    Auth {
        tag: Tag,
        afid: Fid,
        uname: String,
        aname: String,
    },
    #[serde(rename = "104")]
    Attach {
        tag: Tag,
        fid: Fid,
        afid: Fid,
        uname: String,
        aname: String,
    },
    // 106 would be TError, but it doesn't exist
    #[serde(rename = "108")]
    Flush { tag: Tag, oldtag: Tag },
    #[serde(rename = "110")]
    Walk {
        tag: Tag,
        fid: Fid,
        newfid: Fid,
        name: Vec<String>,
    },
    #[serde(rename = "112")]
    Open { tag: Tag, fid: Fid, mode: u8 },
    #[serde(rename = "114")]
    Create {
        tag: Tag,
        fid: Fid,
        name: String,
        perm: u32,
        mode: u8,
    },
    #[serde(rename = "116")]
    Read {
        tag: Tag,
        fid: Fid,
        offset: u32,
        count: u32,
    },
    #[serde(rename = "118")]
    Write {
        tag: Tag,
        fid: Fid,
        offset: u32,
        data: Vec<u8>,
    },
    #[serde(rename = "120")]
    Clunk { tag: Tag, fid: Fid },
    #[serde(rename = "122")]
    Remove { tag: Tag, fid: Fid },
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
pub enum RMessage {
    #[serde(rename = "101")]
    Version {
        tag: Tag,
        msize: u32,
        version: String,
    },
    #[serde(rename = "103")]
    Auth { tag: Tag, aqid: Qid },
    #[serde(rename = "105")]
    Attach { tag: Tag, qid: Qid },
    #[serde(rename = "107")]
    Error { tag: Tag, ename: String },
    #[serde(rename = "109")]
    Flush { tag: Tag },
    #[serde(rename = "111")]
    Walk { tag: Tag, qid: Vec<Qid> },
    #[serde(rename = "113")]
    Open { tag: Tag, qid: Qid, iounit: u32 },
    #[serde(rename = "115")]
    Create {
        tag: Tag,
        qid: Vec<Qid>,
        iounit: u32,
    },
    #[serde(rename = "117")]
    Read { tag: Tag, data: Vec<u8> },
    #[serde(rename = "119")]
    Write { tag: Tag, count: u32 },
    #[serde(rename = "121")]
    Clunk(Tag),
    #[serde(rename = "123")]
    Remove(Tag),
}

impl RMessage {
    pub fn tag(&self) -> Tag {
        *match self {
            RMessage::Version {
                tag,
                msize,
                version,
            } => tag,
            RMessage::Auth { tag, aqid } => tag,
            RMessage::Attach { tag, qid } => tag,
            RMessage::Error { tag, ename } => tag,
            RMessage::Flush { tag } => tag,
            RMessage::Walk { tag, qid } => tag,
            RMessage::Open { tag, qid, iounit } => tag,
            RMessage::Create { tag, qid, iounit } => tag,
            RMessage::Read { tag, data } => tag,
            RMessage::Write { tag, count } => tag,
            RMessage::Clunk(tag) => tag,
            RMessage::Remove(tag) => tag,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
struct DirEnt {
    qid: Qid,
    offset: u64,
    dirent_type: u8,
    name: String,
}

pub struct NineP {
    mtu: usize,
    outbox: channel::Sender<TMessage>,
    inbox: Arc<SpinLock<BTreeMap<Tag, Either<RMessage, Waker>>>>,
}

#[derive(Debug)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

impl NineP {
    pub fn new(connection: Object) -> Result<NineP> {
        let (outbox, outbox_rx) = channel::unbounded();
        let inbox = Arc::new(SpinLock::new(BTreeMap::new()));
        let mut rx = connection.clone();
        let mut tx = connection;
        let mtu = 4096;
        kernel::rt::spawn(async move {
            loop {
                let Ok(out) = outbox_rx.recv().await else {
                    return;
                };
                log::info!("sending: {out:?}");
                let msg = wire::to_bytes_with_len(out).unwrap();
                if !tx.write(&msg).await.is_ok() {
                    return;
                }
            }
        });
        let mtu2 = mtu;
        let inbox2 = inbox.clone();
        kernel::rt::spawn(async move {
            let mtu = mtu2;
            let inbox = inbox2;
            loop {
                let buf = rx.read(mtu).await.unwrap();
                let msg: RMessage = wire::from_bytes_with_len(&buf).unwrap();
                log::info!("got: {msg:?}");
                let mut inbox = inbox.lock();
                let old = inbox.insert(msg.tag(), Either::<RMessage, Waker>::Left(msg));
                if let Some(Either::Right(waker)) = old {
                    waker.wake();
                }
            }
        });
        Ok(NineP { mtu, outbox, inbox })
    }
}

// impl Backend for NineP {
//     fn auth(&self, tag: Tag, afid: Fid, uname: &str, aname: &str) {
//         self.outbox.send_blocking(TMessage::Auth {
//             tag,
//             afid,
//             uname: uname.to_string(),
//             aname: aname.to_string(),
//         });
//     }

//     fn attach(&self, tag: Tag, fid: Fid, afid: Option<Fid>, uname: &str, aname: &str) {
//         self.outbox.send_blocking(TMessage::Attach {
//             tag,
//             fid,
//             afid: afid.unwrap_or(Fid(!0)),
//             uname: uname.to_string(),
//             aname: aname.to_string(),
//         });
//     }

//     fn walk(&self, tag: Tag, dir: Fid, newfid: Fid, path: &Path) {
//         self.outbox.send_blocking(TMessage::Walk {
//             tag,
//             fid: dir,
//             newfid,
//             name: path
//                 .components()
//                 .filter_map(|x| match x {
//                     Component::Normal(x) => Some(x.to_string()),
//                     _ => None,
//                 })
//                 .collect(),
//         });
//     }

//     fn open(&self, tag: Tag, fid: Fid, mode: u8) {
//         self.outbox.send_blocking(TMessage::Open { tag, fid, mode });
//     }

//     fn create(&self, tag: Tag, dir: Fid, name: &str, perm: (), mode: ()) {
//         self.outbox.send_blocking(TMessage::Create {
//             tag,
//             fid: dir,
//             name: name.to_string(),
//             perm: 0,
//             mode: 0,
//         });
//     }

//     fn read(&self, tag: Tag, file: Fid, offset: usize, count: usize) {
//         todo!()
//     }

//     fn write(&self, tag: Tag, file: Fid, offset: usize, data: &[u8]) {
//         todo!()
//     }

//     fn clunk(&self, tag: Tag, fid: Fid) {
//         todo!()
//     }

//     fn remove(&self, tag: Tag, fid: Fid) {
//         todo!()
//     }

//     fn stat(&self, tag: Tag, fid: Fid) {
//         todo!()
//     }

//     fn get_reply_or_wake(&self, tag: Tag, waker: &core::task::Waker) -> Option<crate::fs::Reply> {
//         let mut responses = self.inbox.lock();
//         let resp = match responses.remove(&tag) {
//             Some(Either::Left(l)) => Some(l),
//             None => {
//                 responses.insert(tag, Either::Right(waker.clone()));
//                 None
//             }
//             _ => unreachable!(),
//         }?;
//         Some(match resp {
//             RMessage::Version {
//                 tag,
//                 msize,
//                 version,
//             } => unreachable!(),
//             RMessage::Auth { tag, aqid } => fs::Reply::Auth,
//             RMessage::Attach { tag, qid } => fs::Reply::Attach,
//             RMessage::Error { tag, ename } => fs::Reply::Error(Error::Generic(ename)),
//             RMessage::Flush { tag } => fs::Reply::Flush,
//             RMessage::Walk { tag, qid } => fs::Reply::Walk,
//             RMessage::Open { tag, qid, iounit } => fs::Reply::Open,
//             RMessage::Create { tag, qid, iounit } => fs::Reply::Create,
//             RMessage::Read { tag, data } => fs::Reply::Read(data),
//             RMessage::Write { tag, count } => fs::Reply::Write(count as usize),
//             RMessage::Clunk(tag) => fs::Reply::Clunk,
//             RMessage::Remove(tag) => fs::Reply::Remove,
//         })
//     }

//     fn get_reply(&self, tag: Tag) -> Option<crate::fs::Reply> {
//         todo!()
//     }
// }
