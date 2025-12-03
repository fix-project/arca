use crate::client::file::File9P;
use crate::wire;
use futures::{StreamExt, stream::BoxStream};
use serde::Deserialize;
use vfs::path::Component;

use super::*;

pub struct Dir9P {
    pub(super) conn: Arc<Connection>,
    pub(super) fid: Fid,
    pub(super) qid: Qid,
    pub(super) spawn: Arc<super::SpawnFn<'static>>,
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
        log::info!("readdir called!!!");
        //  A client can list the contents of a directory by reading it, as if it were any other file.
        let tag = self.conn.tag();
        let fid = self.fid;

        // check that im actually a dir
        if !self.qid.flags.contains(Flag::Directory) {
            return Err(Error::from(ErrorKind::NotADirectory));
        }

        log::info!("qid : {:?}", self.qid);

        send!(self.conn; {data, ..} <- Read {
            tag,
            fid,
            offset: 0,
            count: u32::MAX,
        });

        log::info!("first 256 bytes = {:?}", &data[..]);

        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            if offset + 2 > data.len() {
                break; // Not enough data for size field
            }

            // Read the size of this entry
            let size = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;

            log::info!(
                "Parsing directory entry of size {}, data len is {} with offset {}",
                size,
                data.len(),
                offset
            );

            offset += 2; // Move past the size field
            if offset + size > data.len() {
                break; // Entry extends beyond available data
            }

            // Use the wire deserializer for this entry
            let entry_data = &data[offset..offset + size];
            log::info!("Entry data: {:?}", entry_data);
            log::info!("entry data len: {}", entry_data.len());
            match wire::from_bytes_with_len::<types::Stat>(entry_data) {
                Ok(stat) => {
                    log::info!("Wire deserializer succeeded for entry: {:?}", stat);
                    // let is_dir = (stat.mode & 0x80000000) != 0; // DMDIR flag
                    entries.push(DirEnt {
                        name: stat.name,
                        dir: true, // TODO(kmohr)
                    });
                }
                Err(e) => {
                    log::warn!("Wire deserializer failed,: {:?}", e);
                }
            }

            offset += size;
        }

        Ok(futures::stream::iter(entries.into_iter().map(Ok)).boxed())
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
