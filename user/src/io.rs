use core::fmt::Write;

use crate::prelude::*;

pub struct File {
    fd: u32,
}

#[derive(Clone, Debug)]
pub struct Error;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Copy, Clone, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    create: bool,
    truncate: bool,
}

#[derive(Copy, Clone)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

impl File {
    pub fn options() -> OpenOptions {
        OpenOptions::default()
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        let result: Blob = Function::symbolic("read")
            .apply(self.fd)
            .apply(bytes.len() as usize)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        Ok(result.read(0, bytes))
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<usize> {
        let result: Word = Function::symbolic("write")
            .apply(self.fd)
            .apply(bytes)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        let result = result.read() as i64;
        if result < 0 {
            return Err(Error);
        } else {
            return Ok(result as usize);
        }
    }

    pub fn seek(&mut self, from: SeekFrom) -> Result<usize> {
        let (whence, offset) = match from {
            SeekFrom::Start(offset) => (arcane::SEEK_SET, offset as usize),
            SeekFrom::End(offset) => (arcane::SEEK_END, offset as usize),
            SeekFrom::Current(offset) => (arcane::SEEK_CUR, offset as usize),
        };
        let result: Word = Function::symbolic("seek")
            .apply(self.fd)
            .apply(offset)
            .apply(whence)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        let result = result.read() as i64;
        if result < 0 {
            return Err(Error);
        } else {
            return Ok(result as usize);
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        Function::symbolic("close")
            .apply(self.fd)
            .call_with_current_continuation();
    }
}

impl OpenOptions {
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    pub fn open(self, path: &str) -> Result<File> {
        let mut flags = if self.read && self.write {
            arcane::O_RDWR
        } else if self.read {
            arcane::O_RDONLY
        } else if self.write {
            arcane::O_WRONLY
        } else {
            0
        };
        if self.append {
            flags |= arcane::O_APPEND;
        }
        if self.create {
            flags |= arcane::O_CREAT;
        }
        if self.truncate {
            flags |= arcane::O_TRUNC;
        }
        let mode = 0o655;
        let result: Word = Function::symbolic("open")
            .apply(path)
            .apply(flags)
            .apply(mode)
            .call_with_current_continuation()
            .try_into()
            .map_err(|_| Error)?;
        let result = result.read() as i64;
        if result < 0 {
            return Err(Error);
        } else {
            return Ok(File { fd: result as u32 });
        }
    }
}

impl Write for File {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s.as_bytes()).map_err(|_| core::fmt::Error)?;
        Ok(())
    }
}
