use super::*;
use alloc::borrow::ToOwned;
use alloc::vec;
use core::result::Result;
use kernel::types::Word;
use vfs::*;

pub async fn open(
    state: &ProcState,
    path: &[u8],
    flags: OpenFlags,
    mode: ModeT,
) -> Result<u32, UnixError> {
    let s = &str::from_utf8(path)?;
    let path = Path::new(s);
    let open: Open = flags.try_into()?;
    let create: Create = mode.into();
    let path = if path.is_relative() {
        (*state.cwd).clone() + path
    } else {
        path.to_owned()
    };
    let name = path
        .file_name()
        .ok_or(Error::from(ErrorKind::Unsupported))?;
    let path = path.relative();
    let parent = Path::new(path).parent().unwrap();
    let dir = state.ns.walk(parent, Open::Write).await?.as_dir().unwrap();
    let file = if flags.create() {
        dir.create_or_open(name, create, open).await?
    } else {
        dir.open(name, open).await?
    };
    let index = state.fds.write().insert(file.into());
    Ok(index as u32)
}

pub async fn write(state: &ProcState, fd: u64, buf: &[u8]) -> Result<u32, UnixError> {
    let mut fdt = state.fds.write();
    let fd = fdt
        .get_mut(fd as usize)
        .ok_or(UnixError::BADFD)?
        .as_file_mut()?;
    Ok(fd.dyn_write(buf).await? as u32)
}

pub async fn read(state: &ProcState, fd: u64, count: u64) -> Result<Blob, UnixError> {
    log::info!("read: fd={}, count={}", fd as usize, count);
    let mut fdt = state.fds.write();
    let fd = fdt
        .get_mut(fd as usize)
        .ok_or(UnixError::BADFD)?
        .as_file_mut()?;
    log::info!("count={}", count);
    let mut buf = vec![0; count as usize];
    let sz = fd.dyn_read(&mut buf).await?;
    log::info!("sz={}", sz);
    buf.truncate(sz);
    Ok(Blob::from_inner(kernel::types::internal::Blob::new(buf)))
}

pub async fn seek(state: &ProcState, fd: u64, offset: u64, whence: u64) -> Result<Word, UnixError> {
    let mut fdt = state.fds.write();
    let fd = fdt
        .get_mut(fd as usize)
        .ok_or(UnixError::BADFD)?
        .as_file_mut()?;
    let from = match whence as u32 {
        arcane::SEEK_SET => SeekFrom::Start(offset as usize),
        arcane::SEEK_END => SeekFrom::End(offset as isize),
        arcane::SEEK_CUR => SeekFrom::Current(offset as isize),
        _ => return Err(UnixError::INVAL),
    };
    let result = fd.dyn_seek(from).await? as u64;
    Ok(Word::from_inner(result.into()))
}

pub async fn close(state: &ProcState, fd: u64) -> Result<u32, UnixError> {
    let mut fdt = state.fds.write();
    fdt.remove(fd as usize).ok_or(UnixError::BADFD)?;
    Ok(0)
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct OpenFlags(pub u32);

impl TryFrom<OpenFlags> for Open {
    type Error = Error;

    fn try_from(value: OpenFlags) -> vfs::Result<Self> {
        let value = value.0;
        let acc = value & arcane::O_ACCMODE;
        let mut open = match acc {
            arcane::O_RDONLY => Open::Read,
            arcane::O_RDWR => Open::ReadWrite,
            arcane::O_WRONLY => Open::Write,
            _ => return Err(ErrorKind::Unsupported.into()),
        };
        if value & arcane::O_APPEND != 0 {
            open |= Open::Append;
        }
        if value & arcane::O_TRUNC != 0 {
            open |= Open::Truncate;
        }
        Ok(open)
    }
}

impl From<OpenFlags> for Create {
    fn from(value: OpenFlags) -> Self {
        let value = value.0;
        let mut c = Create::default();
        if (value & arcane::O_DIRECTORY) != 0 {
            c |= Create::Directory;
        }
        c
    }
}

impl OpenFlags {
    pub fn create(&self) -> bool {
        (self.0 & arcane::O_CREAT) != 0
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ModeT(pub u32);

impl From<ModeT> for Create {
    fn from(value: ModeT) -> Self {
        let value = value.0;
        Create::from_bits_truncate((value & 0o777) as u64)
    }
}
