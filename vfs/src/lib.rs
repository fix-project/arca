#![no_std]
#![feature(async_trait_bounds)]
#![feature(ptr_metadata)]
#![allow(async_fn_in_trait)]

extern crate alloc;

pub mod error;
pub mod mem;
pub mod path;

use alloc::{boxed::Box, string::String};
use async_trait::async_trait;
use derive_more::{From, TryInto};

pub use error::{Error, ErrorKind, Result};
use futures::stream::BoxStream;
pub use mem::{MemDir, MemFile};
pub use path::{Path, PathBuf};

use bitflags::bitflags;

#[trait_variant::make(Send)]
pub trait File: Send + Sync + DynFile + 'static {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize>
    where
        Self: Sized;
    async fn write(&mut self, bytes: &[u8]) -> Result<usize>
    where
        Self: Sized;
    async fn seek(&mut self, from: SeekFrom) -> Result<usize>
    where
        Self: Sized;
    async fn dup(&self) -> Result<Self>
    where
        Self: Sized;

    fn seekable(&self) -> bool {
        true
    }

    fn boxed(self) -> Box<dyn File>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

#[trait_variant::make(Send)]
pub trait Dir: Send + Sync + DynDir + 'static {
    async fn open(&self, name: &str, open: Open) -> Result<Object>
    where
        Self: Sized;
    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>>
    where
        Self: Sized;
    async fn create(&self, name: &str, create: Create, open: Open) -> Result<Object>
    where
        Self: Sized;
    async fn remove(&self, name: &str) -> Result<()>
    where
        Self: Sized;
    async fn dup(&self) -> Result<Self>
    where
        Self: Sized;

    async fn walk<T: AsRef<Path> + Send>(&self, path: T, open: Open) -> Result<Object>
    where
        Self: Sized,
    {
        log::info!("Dir::walk: path={:?}, open={:?}", path.as_ref(), open);
        async move {
            let path = path.as_ref().relative();
            let (head, rest) = path.split();
            if rest.is_empty() {
                if head.is_empty() {
                    return Ok(Object::Dir(self.dup().await?.boxed()));
                }
                return self.open(head, open).await;
            }
            let child = self.open(head, Open::Read).await?.as_dir()?;
            child.walk(rest, open).await
        }
    }

    fn boxed(self) -> Box<dyn Dir>
    where
        Self: Sized,
    {
        Box::new(self)
    }
}

#[async_trait]
pub trait DynFile {
    async fn dyn_read(&mut self, bytes: &mut [u8]) -> Result<usize>;
    async fn dyn_write(&mut self, bytes: &[u8]) -> Result<usize>;
    async fn dyn_seek(&mut self, from: SeekFrom) -> Result<usize>;
    async fn dyn_dup(&self) -> Result<Box<dyn File>>;
}

#[async_trait]
impl<T: File> DynFile for T {
    async fn dyn_read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        self.read(bytes).await
    }

    async fn dyn_write(&mut self, bytes: &[u8]) -> Result<usize> {
        self.write(bytes).await
    }

    async fn dyn_seek(&mut self, from: SeekFrom) -> Result<usize> {
        self.seek(from).await
    }

    async fn dyn_dup(&self) -> Result<Box<dyn File>> {
        Ok(self.dup().await?.boxed())
    }
}

#[async_trait]
pub trait DynDir {
    async fn dyn_open(&self, name: &str, open: Open) -> Result<Object>;
    async fn dyn_readdir(&self) -> Result<BoxStream<Result<DirEnt>>>;
    async fn dyn_create(&self, name: &str, create: Create, open: Open) -> Result<Object>;
    async fn dyn_remove(&self, name: &str) -> Result<()>;
    async fn dyn_dup(&self) -> Result<Box<dyn Dir>>;
    async fn dyn_walk(&self, path: &Path, open: Open) -> Result<Object>;
}

#[async_trait]
impl<T: Dir> DynDir for T {
    async fn dyn_open(&self, name: &str, open: Open) -> Result<Object> {
        self.open(name, open).await
    }

    async fn dyn_readdir(&self) -> Result<BoxStream<Result<DirEnt>>> {
        self.readdir().await
    }

    async fn dyn_create(&self, name: &str, create: Create, open: Open) -> Result<Object> {
        self.create(name, create, open).await
    }

    async fn dyn_remove(&self, name: &str) -> Result<()> {
        self.remove(name).await
    }

    async fn dyn_dup(&self) -> Result<Box<dyn Dir>> {
        Ok(self.dup().await?.boxed())
    }

    async fn dyn_walk(&self, path: &Path, open: Open) -> Result<Object> {
        self.walk(path, open).await
    }
}

impl File for Box<dyn File> {
    async fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        (**self).dyn_read(bytes).await
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        (**self).dyn_write(bytes).await
    }

    async fn seek(&mut self, from: SeekFrom) -> Result<usize> {
        (**self).dyn_seek(from).await
    }

    async fn dup(&self) -> Result<Self> {
        (**self).dyn_dup().await
    }

    fn seekable(&self) -> bool {
        (**self).seekable()
    }

    fn boxed(self) -> Box<dyn File> {
        self
    }
}

impl Dir for Box<dyn Dir> {
    async fn open(&self, name: &str, open: Open) -> Result<Object> {
        log::info!("Dir::open: name={}, open={:?}", name, open);
        (**self).dyn_open(name, open).await
    }

    async fn readdir(&self) -> Result<BoxStream<'_, Result<DirEnt>>> {
        (**self).dyn_readdir().await
    }

    async fn create(&self, name: &str, create: Create, open: Open) -> Result<Object> {
        log::info!(
            "Dir::create: name={}, create={:?}, open={:?}",
            name,
            create,
            open
        );
        (**self).dyn_create(name, create, open).await
    }

    async fn remove(&self, name: &str) -> Result<()> {
        (**self).dyn_remove(name).await
    }

    async fn dup(&self) -> Result<Self> {
        (**self).dyn_dup().await
    }

    async fn walk<T: AsRef<Path> + Send>(&self, path: T, open: Open) -> Result<Object> {
        (**self).dyn_walk(path.as_ref(), open).await
    }

    fn boxed(self) -> Box<dyn Dir> {
        self
    }
}

#[derive(From, TryInto)]
#[try_into(owned, ref, ref_mut)]
pub enum Object {
    File(Box<dyn File>),
    Dir(Box<dyn Dir>),
}

impl From<derive_more::TryIntoError<Object>> for ErrorKind {
    fn from(value: derive_more::TryIntoError<Object>) -> Self {
        match value.input {
            Object::File(_) => ErrorKind::NotADirectory,
            Object::Dir(_) => ErrorKind::IsADirectory,
        }
    }
}

impl From<derive_more::TryIntoError<Object>> for Error {
    fn from(value: derive_more::TryIntoError<Object>) -> Self {
        Error::from(ErrorKind::from(value))
    }
}

impl From<derive_more::TryIntoError<&Object>> for ErrorKind {
    fn from(value: derive_more::TryIntoError<&Object>) -> Self {
        match value.input {
            Object::File(_) => ErrorKind::NotADirectory,
            Object::Dir(_) => ErrorKind::IsADirectory,
        }
    }
}

impl From<derive_more::TryIntoError<&Object>> for Error {
    fn from(value: derive_more::TryIntoError<&Object>) -> Self {
        Error::from(ErrorKind::from(value))
    }
}

impl From<derive_more::TryIntoError<&mut Object>> for ErrorKind {
    fn from(value: derive_more::TryIntoError<&mut Object>) -> Self {
        match value.input {
            Object::File(_) => ErrorKind::NotADirectory,
            Object::Dir(_) => ErrorKind::IsADirectory,
        }
    }
}

impl From<derive_more::TryIntoError<&mut Object>> for Error {
    fn from(value: derive_more::TryIntoError<&mut Object>) -> Self {
        Error::from(ErrorKind::from(value))
    }
}

impl Object {
    pub fn is_file(&self) -> bool {
        matches!(self, Object::File(_))
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, Object::Dir(_))
    }

    pub fn as_file(self) -> Result<Box<dyn File>> {
        Ok(self.try_into()?)
    }

    pub fn as_file_ref(&self) -> Result<&dyn File> {
        #[allow(clippy::borrowed_box)]
        let b: &Box<dyn File> = self.try_into()?;
        Ok(b)
    }

    pub fn as_file_mut(&mut self) -> Result<&mut dyn File> {
        #[allow(clippy::borrowed_box)]
        let b: &mut Box<dyn File> = self.try_into()?;
        Ok(&mut *b)
    }

    pub fn as_dir(self) -> Result<Box<dyn Dir>> {
        Ok(self.try_into()?)
    }

    pub fn as_dir_ref(&self) -> Result<&dyn Dir> {
        #[allow(clippy::borrowed_box)]
        let b: &Box<dyn Dir> = self.try_into()?;
        Ok(b)
    }

    pub fn as_dir_mut(&mut self) -> Result<&mut dyn Dir> {
        #[allow(clippy::borrowed_box)]
        let b: &mut Box<dyn Dir> = self.try_into()?;
        Ok(&mut *b)
    }

    pub async fn dup(&self) -> Result<Self> {
        Ok(match self {
            Object::File(file) => Object::File(file.dup().await?),
            Object::Dir(dir) => Object::Dir(dir.dup().await?),
        })
    }
}

pub enum SeekFrom {
    Start(usize),
    End(isize),
    Current(isize),
}

#[derive(Clone, Debug)]
pub struct DirEnt {
    pub name: String,
    pub dir: bool,
}

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
    pub struct Open: u64 {
        const Read = 0b01;
        const Write = 0b10;
        const ReadWrite = 0b11;

        const Append = 0x10000;
        const Truncate = 0x20000;
        const _ = !0;
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
    pub struct Create: u64 {
        const UserRead = 0o400;
        const UserWrite = 0o200;
        const UserExecute = 0o100;
        const GroupRead = 0o040;
        const GroupWrite = 0o020;
        const GroupExecute = 0o010;
        const OtherRead = 0o004;
        const OtherWrite = 0o002;
        const OtherExecute = 0o001;

        const Directory = 0x10000;
        const _ = !0;
    }
}

impl Default for Open {
    fn default() -> Self {
        Open::ReadWrite
    }
}

impl Default for Create {
    fn default() -> Self {
        Create::from_bits_truncate(0o644)
    }
}

impl Create {
    pub fn perm(&self) -> u8 {
        (self.bits() & 0o777) as u8
    }

    pub fn dir(&self) -> bool {
        self.contains(Create::Directory)
    }
}

pub trait FileExt: File + Send + Sync {
    fn read_exact(&mut self, mut bytes: &mut [u8]) -> impl Future<Output = Result<()>> + Send
    where
        Self: Sized,
    {
        async move {
            while !bytes.is_empty() {
                let n = self.read(bytes).await?;
                bytes = &mut bytes[n..];
            }
            Ok(())
        }
    }
}

impl<T: File> FileExt for T {}

pub trait DirExt: Dir + Send + Sync {
    fn create_or_open(
        &self,
        name: &str,
        create: Create,
        open: Open,
    ) -> impl Future<Output = Result<Object>> + Send
    where
        Self: Sized,
    {
        async move {
            if let Ok(obj) = self.open(name, open).await {
                return Ok(obj);
            }
            if let Ok(obj) = self.create(name, create, open).await {
                return Ok(obj);
            }
            self.open(name, open).await
        }
    }
}

impl<T: Dir> DirExt for T {}
