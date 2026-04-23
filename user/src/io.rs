use core::{
    fmt::Write,
    marker::PhantomData,
    num::{NonZeroU32, NonZeroUsize},
};

use crate::prelude::*;

extern crate alloc;

pub struct File {
    pub fd: u32,
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
    pub fn open(file: &str) -> Result<File> {
        OpenOptions::default().read(true).open(file)
    }

    pub const STDIN: File = File { fd: 0 };
    pub const STDOUT: File = File { fd: 1 };
    pub const STDERR: File = File { fd: 2 };

    pub fn options() -> OpenOptions {
        OpenOptions::default()
    }

    pub fn read(&mut self, bytes: &mut [u8]) -> Result<usize> {
        let result: Blob = Function::symbolic("read")
            .apply(self.fd)
            .apply(bytes.len())
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
            Err(Error)
        } else {
            Ok(result as usize)
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
            Err(Error)
        } else {
            Ok(result as usize)
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

impl Write for File {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s.as_bytes()).map_err(|_| core::fmt::Error)?;
        Ok(())
    }
}

impl Iterator for File {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let mut bytes = [0];
        crate::error::log("reading");
        if let Ok(1) = self.read(&mut bytes) {
            crate::error::log_int("read", bytes[0] as u64);
            Some(bytes[0])
        } else {
            None
        }
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
            Err(Error)
        } else {
            Ok(File { fd: result as u32 })
        }
    }
}

pub fn exit(code: u8) -> ! {
    Function::symbolic("exit")
        .apply(code as u64)
        .call_with_current_continuation();
    unreachable!()
}

pub fn fork() -> Result<Option<NonZeroUsize>> {
    let result: Word = Function::symbolic("fork")
        .call_with_current_continuation()
        .try_into()
        .map_err(|_| Error)?;
    let result = result.read() as usize;
    Ok(NonZeroUsize::new(result))
}

#[cfg(feature = "allocator")]
mod buf {
    extern crate alloc;
    use core::ops::{Deref, DerefMut};

    use super::*;
    use alloc::vec::Vec;

    pub struct Buffered {
        file: File,
        pending: Vec<u8>,
    }

    impl Buffered {
        pub fn new(file: File) -> Self {
            Self {
                file,
                pending: Vec::new(),
            }
        }

        pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            let n = core::cmp::min(buf.len(), self.pending.len());
            buf[..n].copy_from_slice(&self.pending[..n]);
            self.pending = self.pending[n..].to_vec();
            self.file.read(&mut buf[n..])
        }

        pub fn read_until(&mut self, end: u8) -> Result<Vec<u8>> {
            let mut buffer = [0; 1024];
            loop {
                if let Some(i) = self.pending.iter().position(|x| *x == end) {
                    let head = self.pending[..i + 1].to_vec();
                    let rest = self.pending[i + 1..].to_vec();
                    self.pending = rest;
                    return Ok(head);
                }
                let n = self.file.read(&mut buffer)?;
                let slice = &buffer[..n];
                self.pending.extend_from_slice(slice);
            }
        }
    }

    impl Deref for Buffered {
        type Target = File;

        fn deref(&self) -> &Self::Target {
            &self.file
        }
    }

    impl DerefMut for Buffered {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.file
        }
    }
}

pub use buf::Buffered;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use ::core::fmt::Write;
        ::core::write!(File::STDOUT, $($arg)*).unwrap();
    }};
}

#[macro_export]
macro_rules! println {
    () => {
        {
        use ::core::fmt::Write;
        ::core::writeln!(File::STDOUT).unwrap()
        }
    };
    ($($arg:tt)*) => {
        {
        use ::core::fmt::Write;
        ::core::writeln!(File::STDOUT, $($arg)*).unwrap()
        }
    };
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        use ::core::fmt::Write;
        ::core::write!(File::STDERR).unwrap();
    }};
}

#[macro_export]
macro_rules! eprintln {
    () => {
        {
        use ::core::fmt::Write;
        ::core::writeln!(File::STDERR).unwrap()
        }
    };
    ($($arg:tt)*) => {
        {
        use ::core::fmt::Write;
        ::core::writeln!(File::STDERR, $($arg)*).unwrap()
        }
    };
}
