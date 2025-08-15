use core::str::Utf8Error;

pub mod env;
pub mod file;
pub mod namespace;

use common::util::descriptors::Descriptors;
use derive_more::Display;
pub use env::Env;
pub use namespace::Namespace;

use kernel::{
    prelude::RwLock,
    types::{Blob, Function, Tuple, Value},
};
mod table;
use alloc::{sync::Arc, vec::Vec};
use vfs::{ErrorKind, Object, PathBuf};

pub struct Proc {
    f: Function,
    pid: u64,
    state: Arc<ProcState>,
}

impl Proc {
    pub fn new(elf: &[u8], state: ProcState) -> Result<Self, common::elfloader::Error> {
        let f = common::elfloader::load_elf(elf)?;
        let state = Arc::new(state);
        let pid = table::PROCS.allocate(&state);
        let p = Proc { f, state, pid };
        Ok(p)
    }

    pub async fn run(self, argv: impl IntoIterator<Item = &str>) -> u8 {
        let _argv = Tuple::from_iter(argv.into_iter().map(Blob::from).map(Value::Blob));
        let mut f = self.f;
        loop {
            let result = f.force();
            let Value::Function(g) = result else {
                return 255;
            };
            if g.is_arcane() {
                // call/cc to another function, or returned another function
                f = g;
                continue;
            }
            let data = g.into_inner().read();
            let Value::Tuple(mut data) = data else {
                unreachable!();
            };
            let t: Blob = data.take(0).try_into().unwrap();
            assert_eq!(&*t, b"Symbolic");
            let effect: Blob = data.take(1).try_into().unwrap();
            let args: Tuple = data.take(2).try_into().unwrap();
            let mut args: Vec<Value> = args.into_iter().collect();
            let Some(Value::Function(k)) = args.pop() else {
                return 255;
            };
            f = match (&*effect, &mut *args) {
                (b"open", &mut [Value::Blob(ref path), Value::Word(flags), Value::Word(mode)]) => k
                    .apply(fix(file::open(
                        &self.state,
                        path,
                        file::OpenFlags(flags.read() as u32),
                        file::ModeT(mode.read() as u32),
                    )
                    .await)),
                (b"write", &mut [Value::Word(fd), Value::Blob(ref data)]) => {
                    k.apply(fix(file::write(&self.state, fd.read(), data).await))
                }
                (b"read", &mut [Value::Word(fd), Value::Word(count)]) => {
                    k.apply(fix(file::read(&self.state, fd.read(), count.read()).await))
                }
                (b"seek", &mut [Value::Word(fd), Value::Word(offset), Value::Word(whence)]) => k
                    .apply(fix(file::seek(
                        &self.state,
                        fd.read(),
                        offset.read(),
                        whence.read(),
                    )
                    .await)),
                (b"close", &mut [Value::Word(fd)]) => {
                    k.apply(fix(file::close(&self.state, fd.read()).await))
                }
                (b"exit", &mut [Value::Word(result)]) => {
                    return result.read() as u8;
                }
                _ => {
                    panic!("invalid effect: {effect:?}({args:?})");
                }
            }
        }
    }
}

pub struct ProcState {
    pub ns: Namespace,
    pub env: Env,
    pub fd: RwLock<Descriptors<Object>>,
    pub cwd: PathBuf,
}

#[derive(Debug, Display, Copy, Clone, Eq, PartialEq)]
pub struct UnixError(pub u32);

impl core::error::Error for UnixError {}

impl UnixError {
    pub const BADFD: UnixError = UnixError(arcane::EBADFD);
    pub const ISDIR: UnixError = UnixError(arcane::EISDIR);
    pub const NOTDIR: UnixError = UnixError(arcane::ENOTDIR);
    pub const INVAL: UnixError = UnixError(arcane::EINVAL);
}

impl From<Utf8Error> for UnixError {
    fn from(_: Utf8Error) -> Self {
        UnixError(arcane::EINVAL)
    }
}

fn fix<T: Into<Value>>(value: Result<T, UnixError>) -> Value {
    match value {
        Ok(x) => x.into(),
        Err(x) => Value::Word(((-(x.0 as i64)) as u64).into()),
    }
}

impl From<vfs::Error> for UnixError {
    fn from(value: vfs::Error) -> Self {
        UnixError(match value.kind() {
            ErrorKind::NotFound => arcane::ENOENT,
            ErrorKind::PermissionDenied => arcane::EPERM,
            ErrorKind::AlreadyExists => arcane::EEXIST,
            ErrorKind::NotADirectory => arcane::ENOTDIR,
            ErrorKind::IsADirectory => arcane::EISDIR,
            ErrorKind::DirectoryNotEmpty => arcane::ENOTEMPTY,
            ErrorKind::InvalidInput => arcane::EINVAL,
            ErrorKind::InvalidData => arcane::EINVAL,
            ErrorKind::TimedOut => arcane::ETIMEDOUT,
            ErrorKind::StorageFull => arcane::ENOSPC,
            ErrorKind::NotSeekable => arcane::ESPIPE,
            ErrorKind::QuotaExceeded => arcane::EDQUOT,
            ErrorKind::FileTooLarge => arcane::EFBIG,
            ErrorKind::ResourceBusy => arcane::EBUSY,
            ErrorKind::Deadlock => arcane::EDEADLOCK,
            ErrorKind::CrossesDevices => arcane::EXDEV,
            ErrorKind::InvalidFilename => arcane::EINVAL,
            ErrorKind::ArgumentListTooLong => arcane::E2BIG,
            ErrorKind::Interrupted => arcane::EINTR,
            ErrorKind::Unsupported => arcane::ENOTSUP,
            ErrorKind::UnexpectedEof => arcane::EPIPE,
            ErrorKind::OutOfMemory => arcane::ENOMEM,
            ErrorKind::InProgress => arcane::EAGAIN,
            _ => arcane::EIO,
        })
    }
}

impl From<UnixError> for vfs::Error {
    fn from(value: UnixError) -> Self {
        vfs::Error::other(value)
    }
}
