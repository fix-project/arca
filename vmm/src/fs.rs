use futures::{lock::Mutex, stream::BoxStream, StreamExt};
use ninep::*;
use std::sync::Arc;
use tokio::{
    fs::OpenOptions,
    io::{AsyncReadExt as _, AsyncSeekExt as _, AsyncWriteExt as _},
};
use tokio_stream::wrappers::ReadDirStream;
use vfs::{Create, Dir, DirEnt, ErrorKind, File, Object, Open};

#[derive(Clone)]
pub struct FsDir {
    open: Open,
    path: std::path::PathBuf,
}

#[derive(Clone)]
pub struct FsFile {
    file: Arc<Mutex<tokio::fs::File>>,
}

impl FsDir {
    pub fn new(path: impl AsRef<std::path::Path>, open: Open) -> Option<FsDir> {
        let path = path.as_ref();
        if path.exists() {
            Some(FsDir {
                open,
                path: path.to_owned(),
            })
        } else {
            None
        }
    }
}

impl Dir for FsDir {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let mut path = self.path.clone();
        path.push(name);
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            Err(ErrorKind::NotFound.into())
        } else if path.is_dir() {
            Ok(Object::Dir(FsDir { open, path }.boxed()))
        } else {
            let file = OpenOptions::new()
                .read(open.contains(Open::Read))
                .write(open.contains(Open::Write))
                .append(open.contains(Open::Append))
                .truncate(open.contains(Open::Truncate))
                .create(false)
                .open(path)
                .await
                .map_err(Error::other)?;
            Ok(Object::File(
                FsFile {
                    file: Arc::new(Mutex::new(file)),
                }
                .boxed(),
            ))
        }
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        if !self.open.contains(Open::Read) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        Ok(ReadDirStream::new(
            tokio::fs::read_dir(&self.path)
                .await
                .map_err(Error::other)?,
        )
        .then(async |e| {
            let e = e.map_err(Error::other)?;
            Ok(DirEnt {
                name: e.file_name().to_string_lossy().into_owned(),
                dir: e.file_type().await.map_err(Error::other)?.is_dir(),
            })
        })
        .boxed())
    }

    async fn create(&self, name: &str, create: Create, open: Open) -> Result<Object> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let mut path = self.path.clone();
        path.push(name);
        if create.dir() {
            tokio::fs::create_dir(&path).await.map_err(Error::other)?;
            Ok(Object::Dir(FsDir { open, path }.boxed()))
        } else {
            let file = OpenOptions::new()
                .read(open.contains(Open::Read))
                .write(open.contains(Open::Write))
                .append(open.contains(Open::Append))
                .create_new(true)
                .open(path)
                .await
                .map_err(Error::other)?;
            Ok(Object::File(
                FsFile {
                    file: Arc::new(Mutex::new(file)),
                }
                .boxed(),
            ))
        }
    }

    async fn remove(&self, name: &str) -> Result<()> {
        if !self.open.contains(Open::Write) {
            return Err(ErrorKind::PermissionDenied.into());
        }
        let mut path = self.path.clone();
        path.push(name);
        if path.is_dir() {
            tokio::fs::remove_dir(path).await.map_err(Error::other)?;
        } else {
            tokio::fs::remove_file(path).await.map_err(Error::other)?;
        }
        Ok(())
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}

impl File for FsFile {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        let mut file = self.file.lock().await;
        file.read(bytes).await.map_err(Error::other)
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let mut file = self.file.lock().await;
        file.write(bytes).await.map_err(Error::other)
    }

    async fn seek(&mut self, from: vfs::SeekFrom) -> Result<usize> {
        let mut file = self.file.lock().await;
        file.seek(match from {
            vfs::SeekFrom::Start(x) => std::io::SeekFrom::Start(x as u64),
            vfs::SeekFrom::End(x) => std::io::SeekFrom::End(x as i64),
            vfs::SeekFrom::Current(x) => std::io::SeekFrom::Current(x as i64),
        })
        .await
        .map_err(Error::other)
        .map(|x| x as usize)
    }

    async fn dup(&self) -> Result<Self> {
        Ok(self.clone())
    }
}
