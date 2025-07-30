use core::task::Waker;

use alloc::collections::{btree_map::BTreeMap, btree_set::BTreeSet};
use common::util::rwlock::RwLock;
use kernel::{
    prelude::*,
    virtio::vsock::{Flow, SocketAddr, Stream},
};

use crate::{
    fs::{self, Error, Reply},
    path::Path,
};

use alloc::vec;

#[derive(Default)]
pub struct VSockFS {
    types: RwLock<BTreeMap<fs::Fid, Type>>,
    responses: SpinLock<BTreeMap<fs::Tag, Either<Reply, Waker>>>,
    addrs: RwLock<BTreeSet<SocketAddr>>,
    flows: RwLock<BTreeMap<Flow, Arc<SpinLock<Stream>>>>,
}

#[derive(Debug)]
enum Either<L, R> {
    Left(L),
    Right(R),
}

#[derive(Clone)]
enum Type {
    Auth,
    Root,
    Addr(SocketAddr),
    Flow(Flow),
    Stream(Arc<SpinLock<Stream>>),
}

impl VSockFS {
    pub fn new() -> Self {
        Self::default()
    }

    fn reply(&self, tag: fs::Tag, reply: Reply) {
        let mut responses = self.responses.lock();
        match responses.remove(&tag) {
            Some(Either::Right(waker)) => {
                responses.insert(tag, Either::Left(reply));
                waker.wake();
            }
            None => {
                responses.insert(tag, Either::Left(reply));
            }
            _ => unreachable!(),
        };
    }
}

#[allow(unused_variables)]
impl fs::Backend for VSockFS {
    fn auth(&self, tag: fs::Tag, afid: fs::Fid, uname: &str, aname: &str) {
        let _ = uname;
        let _ = aname;
        let mut types = self.types.write();
        types.insert(afid, Type::Auth);
        self.reply(tag, Reply::Auth);
    }

    fn attach(&self, tag: fs::Tag, fid: fs::Fid, afid: Option<fs::Fid>, uname: &str, aname: &str) {
        let _ = uname;
        let _ = aname;
        let _ = afid;
        let mut types = self.types.write();
        types.insert(fid, Type::Root);
        self.reply(tag, Reply::Attach);
    }

    fn walk(&self, tag: fs::Tag, dir: fs::Fid, newfid: fs::Fid, path: &Path) {
        let mut types = self.types.write();
        let addrs = self.addrs.read();
        let flows = self.flows.read();
        let result = (|| {
            let mut current = types.get(&dir).ok_or(Error::InvalidArgument)?.clone();
            for component in path.components().filter_map(|x| match x {
                crate::path::Component::Normal(x) => Some(x),
                _ => None,
            }) {
                current = match (current, component) {
                    (Type::Root, x) => {
                        let port: u32 = x.parse().map_err(|_| Error::NoSuchFileOrDirectory)?;
                        let addr = SocketAddr { cid: 3, port };
                        if addrs.contains(&addr) {
                            Type::Addr(addr)
                        } else {
                            return Err(Error::NoSuchFileOrDirectory);
                        }
                    }
                    (Type::Addr(src), x) => {
                        let parts = x.split_once(":").ok_or(Error::NoSuchFileOrDirectory)?;
                        let cid: u64 = parts.0.parse().map_err(|_| Error::NoSuchFileOrDirectory)?;
                        let port: u32 =
                            parts.1.parse().map_err(|_| Error::NoSuchFileOrDirectory)?;
                        let dst = SocketAddr { cid, port };
                        let flow = Flow { src, dst };
                        if flows.contains_key(&flow) {
                            Type::Flow(flow)
                        } else {
                            return Err(Error::NoSuchFileOrDirectory);
                        }
                    }
                    (Type::Flow(_), _) => {
                        return Err(Error::NotADirectory);
                    }
                    (Type::Stream(_), _) => {
                        return Err(Error::NotADirectory);
                    }
                    _ => {
                        return Err(Error::NoSuchFileOrDirectory);
                    }
                }
            }
            types.insert(newfid, current);
            Ok(())
        })();
        match result {
            Ok(()) => self.reply(tag, Reply::Walk),
            Err(e) => self.reply(tag, Reply::Error(e)),
        }
    }

    fn open(&self, tag: fs::Tag, fid: fs::Fid, mode: u8) {
        let mut types = self.types.write();
        let mut flows = self.flows.write();
        let result = (|| {
            let mut current = types.get(&fid).ok_or(Error::InvalidArgument)?;
            let t = match current {
                Type::Flow(flow) => {
                    let stream = flows
                        .get(&flow)
                        .cloned()
                        .or_else(|| {
                            kernel::rt::spawn_blocking(Stream::connect(flow.src, flow.dst))
                                .ok()
                                .map(SpinLock::new)
                                .map(Arc::new)
                        })
                        .ok_or(Error::Generic("could not open connection".into()))?;
                    Type::Stream(stream)
                }
                _ => return Err(Error::IsADirectory),
            };
            types.insert(fid, t);
            Ok(())
        })();
        match result {
            Ok(()) => self.reply(tag, Reply::Walk),
            Err(e) => self.reply(tag, Reply::Error(e)),
        }
    }

    fn create(&self, tag: fs::Tag, dir: fs::Fid, name: &str, perm: (), mode: ()) {
        self.reply(tag, Reply::Error(Error::OperationNotPermitted));
    }

    fn read(&self, tag: fs::Tag, file: fs::Fid, offset: usize, count: usize) {
        let types = self.types.read();
        todo!();
    }

    fn write(&self, tag: fs::Tag, file: fs::Fid, offset: usize, data: &[u8]) {
        let types = self.types.read();
        todo!();
    }

    fn clunk(&self, tag: fs::Tag, fid: fs::Fid) {
        let mut types = self.types.write();
        if types.remove(&fid).is_some() {
            self.reply(tag, Reply::Clunk);
        } else {
            self.reply(tag, Reply::Error(Error::InvalidArgument));
        }
    }

    fn remove(&self, tag: fs::Tag, fid: fs::Fid) {
        todo!()
    }

    fn stat(&self, tag: fs::Tag, fid: fs::Fid) {
        todo!()
    }

    fn get_reply_or_wake(&self, tag: fs::Tag, waker: &core::task::Waker) -> Option<Reply> {
        let mut responses = self.responses.lock();
        match responses.remove(&tag) {
            Some(Either::Left(l)) => Some(l),
            None => {
                responses.insert(tag, Either::Right(waker.clone()));
                None
            }
            _ => unreachable!(),
        }
    }

    fn get_reply(&self, tag: fs::Tag) -> Option<Reply> {
        todo!()
    }
}
