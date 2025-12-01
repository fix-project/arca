use core::{
    cell::UnsafeCell,
    str::Utf8Error,
    sync::atomic::{AtomicBool, Ordering},
    task::{Poll, Waker},
};

pub mod env;
pub mod file;
pub mod namespace;

use arca::{Runtime, Word};
use common::ipaddr::IpAddr;
use common::util::{descriptors::Descriptors, semaphore::Semaphore};
use derive_more::{Display, From, TryInto};
pub use env::Env;
use lz4_flex::block::{compress_prepend_size, decompress_size_prepended};
pub use namespace::Namespace;
use vfs::path::Path;

use kernel::{
    kvmclock,
    prelude::RwLock,
    types::{Blob, Function, Tuple, Value},
};
mod table;
use alloc::{boxed::Box, collections::vec_deque::VecDeque, sync::Arc, vec::Vec};
use vfs::{Dir, ErrorKind, File, Object, PathBuf, Result};

pub struct Proc {
    f: Function,
    pid: u64,
    state: Arc<ProcState>,
}

impl Proc {
    pub fn new(
        elf: &[u8],
        state: ProcState,
    ) -> core::result::Result<Self, common::elfloader::Error> {
        let f = common::elfloader::load_elf(elf)?;
        let state = Arc::new(state);
        let pid = table::PROCS.allocate(&state);
        let p = Proc { f, pid, state };
        Ok(p)
    }

    pub fn from_function(
        function: Function,
        state: ProcState,
    ) -> core::result::Result<Self, common::elfloader::Error> {
        let f = function;
        let state = Arc::new(state);
        let pid = table::PROCS.allocate(&state);
        let p = Proc { f, pid, state };
        Ok(p)
    }

    pub async fn run(self, argv: impl IntoIterator<Item = &str>) -> u8 {
        let _argv = Tuple::from_iter(argv.into_iter().map(Blob::from).map(Value::Blob));
        self.resume().await
    }

    #[allow(clippy::manual_async_fn)]
    pub fn resume(self) -> impl Future<Output = u8> + Send {
        async move {
            let mut f = self.f;
            loop {
                let result = f.force();
                let Value::Function(g) = result else {
                    log::error!("proc returned something other than an effect!");
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
                    (
                        b"open",
                        &mut [Value::Blob(ref path), Value::Word(flags), Value::Word(mode)],
                    ) => {
                        // TODO this is not the right place for all this logic
                        // Figure out which machine the file is on
                        // filenames will be of the form "hostname:port/path/to/file"
                        let tcp_init = kvmclock::time_since_boot();
                        let s = &str::from_utf8(path).unwrap();
                        let path_p = Path::new(s);
                        let (host, filename) = path_p.split();
                        let host_ip: IpAddr = host.try_into().unwrap();
                        if host_ip != *self.state.host {
                            let send_tcp_msg = || async {
                                // Get a new TCP connection
                                let tcp_ctl_result = self
                                    .state
                                    .ns
                                    .walk("/net/tcp/clone", vfs::Open::ReadWrite)
                                    .await;
                                let mut tcp_ctl = match tcp_ctl_result {
                                    Ok(obj) => match obj.as_file() {
                                        Ok(file) => file,
                                        Err(_) => {
                                            log::error!("Failed to get TCP control file");
                                            return 1;
                                        }
                                    },
                                    Err(e) => {
                                        log::error!("Failed to walk to /net/tcp/clone: {:?}", e);
                                        return 1;
                                    }
                                };

                                // Read the connection ID
                                let mut id_buf = [0u8; 32];
                                let size = match tcp_ctl.read(&mut id_buf).await {
                                    Ok(s) => s,
                                    Err(e) => {
                                        log::error!("Failed to read connection ID: {:?}", e);
                                        return 1;
                                    }
                                };
                                let conn_id = match core::str::from_utf8(&id_buf[..size]) {
                                    Ok(s) => s.trim(),
                                    Err(_) => {
                                        log::error!("Invalid UTF-8 in connection ID");
                                        return 1;
                                    }
                                };

                                // Connect to localhost:11234
                                let connect_cmd = alloc::format!("connect 127.0.0.1:11212\n");
                                if let Err(e) = tcp_ctl.write(connect_cmd.as_bytes()).await {
                                    log::error!("Failed to send connect command: {:?}", e);
                                    return 1;
                                }

                                // Get the data file for this connection
                                let data_path = alloc::format!("/net/tcp/{}/data", conn_id);
                                let mut data_file = match self
                                    .state
                                    .ns
                                    .walk(&data_path, vfs::Open::ReadWrite)
                                    .await
                                {
                                    Ok(obj) => match obj.as_file() {
                                        Ok(file) => file,
                                        Err(_) => {
                                            log::error!("Failed to get data file");
                                            return 1;
                                        }
                                    },
                                    Err(e) => {
                                        log::error!("Failed to walk to data path: {:?}", e);
                                        return 1;
                                    }
                                };

                                // TODO(kmohr): hmmmm this is so slow...
                                let k_create_init = kvmclock::time_since_boot();
                                let resumed_f = Function::symbolic("open")
                                    .apply(Value::Blob(path.clone()))
                                    .apply(Value::Word(flags))
                                    .apply(Value::Word(mode))
                                    .apply(Value::Function(k));

                                let val = Value::Function(resumed_f);

                                let serialize_init = kvmclock::time_since_boot();
                                let msg = postcard::to_allocvec(&val).unwrap();
                                let serialize_end = kvmclock::time_since_boot();
                                let msg = compress_prepend_size(&msg);
                                let compress_end = kvmclock::time_since_boot();

                                // log the message size in bytes and MB
                                let size_bytes = msg.len();
                                let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
                                log::info!(
                                    "PERF\ntcp time: {} us\ncontinuation creation: {} us\nserialization: {} us\ncompression: {} us\nsize: {} bytes ({:.2} MB)",
                                    (k_create_init - tcp_init).as_micros(),
                                    (serialize_init - k_create_init).as_micros(),
                                    (serialize_end - serialize_init).as_micros(),
                                    (compress_end - serialize_end).as_micros(),
                                    size_bytes,
                                    size_mb
                                );

                                if let Err(e) = data_file.write(&msg).await {
                                    log::error!("Failed to send msg: {:?}", e);
                                    return 1;
                                }

                                return 0;
                            };

                            let ret_code = send_tcp_msg().await;
                            return ret_code as u8;
                        } else {
                            k.apply(fix(file::open(
                                &self.state,
                                filename.as_bytes(),
                                file::OpenFlags(flags.read() as u32),
                                file::ModeT(mode.read() as u32),
                            )
                            .await))
                        }
                    }
                    (b"write", &mut [Value::Word(fd), Value::Blob(ref data)]) => {
                        k.apply(fix(file::write(&self.state, fd.read(), data).await))
                    }
                    (b"read", &mut [Value::Word(fd), Value::Word(count)]) => {
                        k.apply(fix(file::read(&self.state, fd.read(), count.read()).await))
                    }
                    (b"seek", &mut [Value::Word(fd), Value::Word(offset), Value::Word(whence)]) => {
                        k.apply(fix(file::seek(
                            &self.state,
                            fd.read(),
                            offset.read(),
                            whence.read(),
                        )
                        .await))
                    }
                    (b"close", &mut [Value::Word(fd)]) => {
                        k.apply(fix(file::close(&self.state, fd.read()).await))
                    }
                    (b"dup", &mut [Value::Word(fd)]) => {
                        #[allow(clippy::redundant_closure_call)]
                        let result = (async || {
                            let mut fds = self.state.fds.lock();
                            let old = fds.get(fd.read() as usize).ok_or(UnixError::BADFD)?;
                            let new = old.dup().await?;
                            let fd = fds.insert(new);
                            Ok(Word::new(fd as u64))
                        })()
                        .await;
                        k.apply(fix(result))
                    }
                    (b"fork", &mut []) => {
                        let state = Arc::new((*self.state).clone());
                        let pid = table::PROCS.allocate(&state);
                        let mut fds = Descriptors::new();
                        for (i, x) in self.state.fds.read().iter() {
                            fds.set(i, x.dup().await.unwrap());
                        }
                        let new = Proc {
                            f: k.clone().apply(0),
                            pid,
                            state: Arc::new(ProcState {
                                fds: RwLock::new(fds).into(),
                                ..(*self.state).clone()
                            }),
                        };
                        kernel::rt::spawn(async move { new.resume().await });
                        k.apply(fix(Ok(pid as u32)))
                    }
                    (b"exit", &mut [Value::Word(result)]) => {
                        return result.read() as u8;
                    }
                    (b"monitor-new", &mut []) => {
                        let mvar = MVar::new(Monitor::new());
                        let i = self.state.fds.lock().insert(FileDescriptor::MVar(mvar));
                        k.apply(i as u64)
                    }
                    (b"monitor-enter", &mut [Value::Word(fd), Value::Function(ref mut g)]) => {
                        let fds = self.state.fds.lock();
                        let monitor = fds.get(fd.read() as usize).ok_or(UnixError::BADFD).unwrap();
                        let monitor = monitor.as_mvar_ref().unwrap();
                        k.apply(monitor.run(core::mem::take(g)).await)
                    }
                    _ => {
                        panic!("invalid effect: {effect:?}({args:?})");
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct ProcState {
    pub ns: Arc<Namespace>,
    pub env: Arc<Env>,
    pub fds: Arc<RwLock<Descriptors<FileDescriptor>>>,
    pub cwd: Arc<PathBuf>,
    pub host: Arc<IpAddr>,
}

pub struct Monitor {
    sem: Semaphore,
    value: UnsafeCell<Value>,
    on_change: UnsafeCell<VecDeque<Waker>>,
}

unsafe impl Sync for Monitor {}
unsafe impl Send for Monitor {}

impl Monitor {
    pub fn new() -> Self {
        let sem = Semaphore::new(1);
        Self {
            sem,
            value: Default::default(),
            on_change: Default::default(),
        }
    }

    pub async fn run(&self, mut f: Function) -> Value {
        self.sem.acquire(1).await;
        let mut changed = false;
        loop {
            let result = f.force();
            let Value::Function(g) = result else {
                log::error!("monitor returned something other than an effect!");
                self.sem.release(1);
                return Runtime::create_null().into();
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
                return Runtime::create_null().into();
            };
            f = match (&*effect, &mut *args) {
                (b"get", &mut []) => {
                    let value = unsafe { (*self.value.get()).clone() };
                    k.apply(value)
                }
                (b"set", &mut [ref mut value]) => {
                    changed = true;
                    #[allow(clippy::swap_ptr_to_ref)]
                    unsafe {
                        core::mem::swap(&mut *self.value.get(), value)
                    };
                    k.apply(core::mem::take(value))
                }
                (b"exit", &mut [ref mut value]) => {
                    if changed {
                        unsafe {
                            for waker in (*self.on_change.get()).drain(..) {
                                waker.wake();
                            }
                        }
                    }
                    self.sem.release(1);
                    return core::mem::take(value);
                }
                (b"wait", &mut []) => {
                    let future = WaitFuture {
                        already_fired: AtomicBool::new(false),
                        monitor: self,
                    };
                    future.await;
                    self.sem.acquire(1).await;
                    changed = false;
                    k.apply(Runtime::create_null())
                }
                _ => {
                    self.sem.release(1);
                    return Runtime::create_null().into();
                }
            };
        }
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

struct WaitFuture<'a> {
    already_fired: AtomicBool,
    monitor: &'a Monitor,
}

impl Future for WaitFuture<'_> {
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if self
            .already_fired
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            unsafe {
                (*self.monitor.on_change.get()).push_back(cx.waker().clone());
                self.monitor.sem.release(1);
                return Poll::Pending;
            }
        }
        Poll::Ready(())
    }
}

type MVar = Arc<Monitor>;

#[derive(From, TryInto)]
#[try_into(owned, ref, ref_mut)]
pub enum FileDescriptor {
    File(Box<dyn File>),
    Dir(Box<dyn Dir>),
    MVar(MVar),
}

impl From<Object> for FileDescriptor {
    fn from(value: Object) -> Self {
        match value {
            Object::File(file) => file.into(),
            Object::Dir(dir) => dir.into(),
        }
    }
}

impl FileDescriptor {
    pub fn is_file(&self) -> bool {
        matches!(self, FileDescriptor::File(_))
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, FileDescriptor::Dir(_))
    }

    pub fn into_file(self) -> Result<Box<dyn File>> {
        Ok(self.try_into().map_err(|_| ErrorKind::InvalidInput)?)
    }

    pub fn as_file_ref(&self) -> Result<&dyn File> {
        #[allow(clippy::borrowed_box)]
        let b: &Box<dyn File> = self.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        Ok(b)
    }

    pub fn as_file_mut(&mut self) -> Result<&mut dyn File> {
        let b: &mut Box<dyn File> = self.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        Ok(&mut *b)
    }

    pub fn into_dir(self) -> Result<Box<dyn Dir>> {
        Ok(self.try_into().map_err(|_| ErrorKind::InvalidInput)?)
    }

    pub fn as_dir_ref(&self) -> Result<&dyn Dir> {
        #[allow(clippy::borrowed_box)]
        let b: &Box<dyn Dir> = self.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        Ok(b)
    }

    pub fn as_dir_mut(&mut self) -> Result<&mut dyn Dir> {
        let b: &mut Box<dyn Dir> = self.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        Ok(&mut *b)
    }

    pub fn into_mvar(self) -> Result<MVar> {
        Ok(self.try_into().map_err(|_| ErrorKind::InvalidInput)?)
    }

    pub fn as_mvar_ref(&self) -> Result<&MVar> {
        let b: &MVar = self.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        Ok(b)
    }

    pub fn as_mvar_mut(&mut self) -> Result<&mut MVar> {
        let b: &mut MVar = self.try_into().map_err(|_| ErrorKind::InvalidInput)?;
        Ok(b)
    }

    pub async fn dup(&self) -> Result<Self> {
        Ok(match self {
            FileDescriptor::File(file) => FileDescriptor::File(file.dup().await?),
            FileDescriptor::Dir(dir) => FileDescriptor::Dir(dir.dup().await?),
            FileDescriptor::MVar(mvar) => FileDescriptor::MVar(mvar.clone()),
        })
    }
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

fn fix<T: Into<Value>>(value: core::result::Result<T, UnixError>) -> Value {
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
