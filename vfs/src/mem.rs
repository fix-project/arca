use super::*;
use alloc::{
    collections::btree_map::BTreeMap, string::String, string::ToString, sync::Arc, vec::Vec,
};
use common::util::rwlock::RwLock;
use either::Either;
use futures::StreamExt as _;

#[derive(Clone, Default)]
struct DirContents {
    data: Arc<RwLock<BTreeMap<String, Either<DirContents, FileContents>>>>,
}

#[derive(Clone, Default)]
struct FileContents {
    data: Arc<RwLock<Vec<u8>>>,
}

#[derive(Clone, Default)]
pub struct MemDir {
    open: Open,
    contents: DirContents,
}

#[derive(Clone, Default)]
pub struct MemFile {
    cursor: usize,
    open: Open,
    contents: FileContents,
}

impl Dir for MemDir {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        log::info!("MemDir::open: name={}, open={:?}", name, open);
        let contents = self.contents.data.read();
        let file = contents.get(name).ok_or(ErrorKind::NotFound)?.clone();
        match file {
            Either::Left(contents) => Ok(Object::Dir(MemDir { open, contents }.boxed())),
            Either::Right(contents) => {
                if open.contains(Open::Truncate) {
                    contents.data.write().truncate(0);
                }
                Ok(Object::File(
                    MemFile {
                        cursor: 0,
                        open,
                        contents,
                    }
                    .boxed(),
                ))
            }
        }
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let contents = self.contents.data.read();

        let dirents: Vec<Result<DirEnt>> = contents
            .iter()
            .map(|(k, v)| {
                Ok(DirEnt {
                    name: k.clone(),
                    dir: v.is_left(),
                })
            })
            .collect();
        Ok(futures::stream::iter(dirents).boxed())
    }

    async fn create(&self, name: &str, create: Create, open: Open) -> Result<Object> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let mut contents = self.contents.data.write();
        if contents.contains_key(name) {
            return Err(ErrorKind::AlreadyExists.into());
        }
        if create.contains(Create::Directory) {
            let file = MemDir {
                open,
                contents: Default::default(),
            };
            contents.insert(name.to_string(), Either::Left(file.contents.clone()));
            Ok(Object::Dir(file.boxed()))
        } else {
            let file = MemFile {
                cursor: 0,
                open,
                contents: Default::default(),
            };
            contents.insert(name.to_string(), Either::Right(file.contents.clone()));
            Ok(Object::File(file.boxed()))
        }
    }

    async fn remove(&self, name: &str) -> Result<()> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let mut contents = self.contents.data.write();
        if contents.remove(name).is_none() {
            Err(ErrorKind::NotFound.into())
        } else {
            Ok(())
        }
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

impl File for MemFile {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        log::info!(
            "MemFile::read: cursor={}, bytes_len={}",
            self.cursor,
            bytes.len()
        );
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let contents = self.contents.data.read();
        let lo = core::cmp::min(contents.len(), self.cursor);
        let hi = core::cmp::min(contents.len(), lo + bytes.len());
        let n = hi - lo;
        bytes[..n].copy_from_slice(&contents[lo..hi]);
        self.cursor += n;
        Ok(n)
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let mut contents = self.contents.data.write();
        if self.open.contains(Open::Append) {
            self.cursor = contents.len();
        }
        if contents.len() < self.cursor + bytes.len() {
            contents.resize(self.cursor + bytes.len(), 0);
        }
        contents[self.cursor..].copy_from_slice(bytes);
        self.cursor += bytes.len();
        Ok(bytes.len())
    }

    async fn seek(&mut self, from: SeekFrom) -> Result<usize> {
        match from {
            SeekFrom::Start(offset) => self.cursor = offset,
            SeekFrom::End(offset) => {
                self.cursor = self
                    .contents
                    .data
                    .read()
                    .len()
                    .saturating_add_signed(offset)
            }
            SeekFrom::Current(offset) => self.cursor = self.cursor.saturating_add_signed(offset),
        }
        let mut contents = self.contents.data.write();
        if self.cursor >= contents.len() {
            contents.resize(self.cursor, 0);
        }
        Ok(self.cursor)
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}
