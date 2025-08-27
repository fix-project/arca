use core::{future::Future, pin::Pin};

use crate::client::file::File9P;
use futures::stream::BoxStream;
use vfs::path::Component;

use super::*;

pub struct Dir9P {
    pub(super) conn: Arc<Connection>,
    pub(super) fid: Fid,
    pub(super) qid: Qid,
    pub(super) spawn: Arc<dyn Fn(Pin<Box<dyn Future<Output = ()> + Send>>) + 'static + Send + Sync>,
}

impl Dir9P {
    pub fn qid(&self) -> Qid {
        self.qid
    }
}

impl Dir for Dir9P {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        self.walk(name, open).await
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        todo!()
    }

    async fn create(&self, name: &str, create: Create, open: Open) -> Result<Object> {
        let mut new = self.dup().await?;
        let tag = new.conn.tag();
        let fid = new.fid;

        let mode = create.into();
        let access = open.into();

        send!(new.conn; {qid, ..} <- Create {
            tag,
            fid,
            name: name.to_owned(),
            mode,
            access
        });

        if qid.flags.contains(Flag::Directory) {
            new.qid = qid;
            Ok(Object::Dir(new.boxed()))
        } else {
            let file = File9P {
                conn: new.conn.clone(),
                fid: new.fid,
                qid,
                cursor: 0,
                spawn: self.spawn.clone(),
            };
            new.fid = Fid(!0);
            Ok(Object::File(file.boxed()))
        }
    }

    async fn remove(&self, name: &str) -> Result<()> {
        let tag = self.conn.tag();
        let fid = self.fid;
        let newfid = self.conn.fid();
        let name = vec![name.to_owned()];
        send!(self.conn; { qid, .. } <- Walk { tag, fid, newfid, name });
        if qid.len() != 1 {
            return Err(Error::from(ErrorKind::NotFound));
        }
        let tag = self.conn.tag();
        send!(self.conn; (_) <- Remove { tag, fid: newfid });
        Ok(())
    }

    async fn dup(&self) -> Result<Self> {
        let tag = self.conn.tag();
        let fid = self.fid;
        let newfid = self.conn.fid();
        send!(self.conn; {qid, ..} <- Walk {tag, fid, newfid, name: vec![]});
        let qid = *qid.last().ok_or(Error::from(ErrorKind::Other))?;
        Ok(Dir9P {
            conn: self.conn.clone(),
            fid: newfid,
            qid,
            spawn: self.spawn.clone(),
        })
    }

    async fn walk<T: AsRef<Path> + Send>(&self, path: T, flags: Open) -> Result<Object> {
        let tag = self.conn.tag();
        let fid = self.fid;
        let newfid = self.conn.fid();
        let name: Vec<String> = path
            .as_ref()
            .components()
            .map(|x| match x {
                Component::RootDir => Some("/".to_owned()),
                Component::CurDir => None,
                Component::ParentDir => None,
                Component::Normal(x) => Some(x.to_owned()),
            })
            .try_collect()
            .ok_or(Error::from(ErrorKind::InvalidFilename))?;
        let n = name.len();
        send!(self.conn; {qid, ..} <- Walk {tag, fid, newfid, name});
        if n > 0 && qid.len() != n {
            return Err(Error::from(ErrorKind::NotFound));
        }
        let qid = *qid.last().unwrap_or(&self.qid);

        if qid.flags.contains(Flag::Directory) {
            let dir = Dir9P {
                conn: self.conn.clone(),
                fid: newfid,
                qid,
                spawn: self.spawn.clone(),
            };
            Ok(Object::Dir(dir.boxed()))
        } else {
            let file = File9P {
                conn: self.conn.clone(),
                fid: newfid,
                qid,
                cursor: 0,
                spawn: self.spawn.clone(),
            };
            let access = flags.into();
            let tag = self.conn.tag();
            send!(file.conn; {..} <- Open { tag, fid: newfid, access });
            Ok(Object::File(file.boxed()))
        }
    }
}

impl Drop for Dir9P {
    fn drop(&mut self) {
        let fid = self.fid;
        let conn = self.conn.clone();
        if self.fid != Fid(!0) {
            let future = Box::pin(async move {
                let tag = conn.tag();
                let _ = conn.send(TMessage::Clunk { tag, fid }).await;
            });
            (self.spawn)(future);
        }
    }
}
