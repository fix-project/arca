use alloc::collections::btree_map::BTreeMap;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use kernel::prelude::*;

mod dev;
mod fs;
mod mem;
mod path;

#[allow(unused_imports)]
pub use self::{
    dev::DevFS,
    fs::{Filesystem, MountType},
    mem::{VDir, VFile},
    path::{Component, Components, Path, PathBuf},
};

use enumflags2::{BitFlag, BitFlags, bitflags};

#[derive(Debug, Clone, Eq, PartialEq)]
#[allow(unused)]
pub enum Error {
    OperationNotPermitted,
    NoSuchFileOrDirectory,
    NoSuchProcess,
    InterruptedSystemCall,
    InputOutputError,
    NoSuchDeviceOrAddress,
    ArgumentListTooLong,
    ExecFormatError,
    BadFileDescriptor,
    NoChildProcesses,
    ResourceTemporarilyUnavailable,
    CannotAllocateMemory,
    PermissionDenied,
    BadAddress,
    BlockDeviceRequired,
    DeviceOrResourceBusy,
    FileExists,
    InvalidCrossDeviceLink,
    NoSuchDevice,
    NotADirectory,
    IsADirectory,
    InvalidArgument,
    TooManyOpenFilesInSystem,
    TooManyOpenFiles,
    InappropriateIoctlForDevice,
    TextFileBusy,
    FileTooLarge,
    NoSpaceLeftOnDevice,
    IllegalSeek,
    ReadOnlyFileSystem,
    TooManyLinks,
    BrokenPipe,
    Code(u32),
    Message(String),
}

pub type Result<T> = core::result::Result<T, Error>;

#[async_trait]
pub trait FileLike: Send + Sync {
    async fn open(&mut self, config: Flags) -> Result<()>;
    async fn read(&self, offset: usize, buf: &mut [u8]) -> Result<usize>;
    async fn write(&mut self, offset: usize, buf: &[u8]) -> Result<usize>;
    async fn close(self) -> Result<()>;
    async fn remove(self) -> Result<()>;
    async fn stat(&self) -> Result<Stat>;
    async fn duplicate(&self) -> Result<File>;
}

#[async_trait]
pub trait DirLike: Send + Sync {
    async fn open(&mut self, config: Flags) -> Result<()>;
    async fn create(&mut self, name: &str, config: Mode) -> Result<Object>;
    async fn read(&self, offset: usize, count: usize) -> Result<Vec<DirEnt>>;
    async fn walk(&self, path: &Path) -> Result<Object>;
    async fn close(self) -> Result<()>;
    async fn remove(self) -> Result<()>;
    async fn stat(&self) -> Result<Stat>;
}

pub type File = Box<dyn FileLike>;
pub type Dir = Box<dyn DirLike>;

pub enum Object {
    File(File),
    Dir(Dir),
}

impl Object {
    pub async fn open(&mut self, config: Flags) -> Result<()> {
        match self {
            Object::File(f) => f.open(config).await,
            Object::Dir(d) => d.open(config).await,
        }
    }

    pub async fn walk(&self, path: &Path) -> Result<Object> {
        match self {
            Object::File(f) => {
                if path.is_empty() {
                    Ok(Object::File(f.duplicate().await?))
                } else {
                    Err(Error::NotADirectory)
                }
            }
            Object::Dir(d) => d.walk(path).await,
        }
    }

    pub async fn stat(&self) -> Result<Stat> {
        match self {
            Object::File(f) => f.stat().await,
            Object::Dir(d) => d.stat().await,
        }
    }

    pub fn is_file(&self) -> bool {
        match self {
            Object::File(_) => true,
            Object::Dir(_) => false,
        }
    }

    pub fn is_dir(&self) -> bool {
        match self {
            Object::File(_) => false,
            Object::Dir(_) => true,
        }
    }
}

impl TryInto<File> for Object {
    type Error = Error;

    fn try_into(self) -> core::result::Result<File, Self::Error> {
        match self {
            Object::File(file) => Ok(file),
            Object::Dir(_) => Err(Error::IsADirectory),
        }
    }
}

impl TryInto<Dir> for Object {
    type Error = Error;

    fn try_into(self) -> core::result::Result<Dir, Self::Error> {
        match self {
            Object::File(_) => Err(Error::NotADirectory),
            Object::Dir(dir) => Ok(dir),
        }
    }
}

impl<'a> TryInto<&'a File> for &'a Object {
    type Error = Error;

    fn try_into(self) -> core::result::Result<&'a File, Self::Error> {
        match self {
            Object::File(file) => Ok(&*file),
            Object::Dir(_) => Err(Error::IsADirectory),
        }
    }
}

impl<'a> TryInto<&'a Dir> for &'a Object {
    type Error = Error;

    fn try_into(self) -> core::result::Result<&'a Dir, Self::Error> {
        match self {
            Object::File(_) => Err(Error::NotADirectory),
            Object::Dir(dir) => Ok(&*dir),
        }
    }
}

impl<'a> TryInto<&'a mut File> for &'a mut Object {
    type Error = Error;

    fn try_into(self) -> core::result::Result<&'a mut File, Self::Error> {
        match self {
            Object::File(file) => Ok(&mut *file),
            Object::Dir(_) => Err(Error::IsADirectory),
        }
    }
}

impl<'a> TryInto<&'a mut Dir> for &'a mut Object {
    type Error = Error;

    fn try_into(self) -> core::result::Result<&'a mut Dir, Self::Error> {
        match self {
            Object::File(_) => Err(Error::NotADirectory),
            Object::Dir(dir) => Ok(&mut *dir),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DirEnt {
    pub name: String,
    pub perm: Perms,
    pub kind: BitFlags<Kind>,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct Flags {
    pub access: Access,
    pub truncate: bool,
    pub rclose: bool,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub struct Mode {
    pub open: Flags,
    pub perm: Perms,
    pub kind: BitFlags<Kind>,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Default)]
pub enum Access {
    Read,
    Write,
    #[default]
    ReadWrite,
    Execute,
}

impl Access {
    pub fn read(&self) -> bool {
        *self == Access::Read || *self == Access::ReadWrite
    }

    pub fn write(&self) -> bool {
        *self == Access::Write || *self == Access::ReadWrite
    }

    pub fn execute(&self) -> bool {
        *self == Access::Execute
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Perms {
    owner: BitFlags<Perm>,
    group: BitFlags<Perm>,
    others: BitFlags<Perm>,
}

impl Default for Perms {
    fn default() -> Self {
        Self {
            owner: Perm::all(),
            group: Perm::all(),
            others: Perm::all(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[bitflags]
#[repr(u8)]
pub enum Perm {
    Read = 1,
    Write = 2,
    Execute = 4,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
#[bitflags]
#[repr(u8)]
pub enum Kind {
    Directory,
    AppendOnly,
    Exclusive,
    Authentication,
    Temporary,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Stat {
    _type: u16,
    _dev: u32,
    version: u32,
    path: u64,
    mode: Mode,
    atime: DateTime<Utc>,
    mtime: DateTime<Utc>,
    length: usize,
    name: String,
    uid: String,
    gid: String,
    muid: String,
}
