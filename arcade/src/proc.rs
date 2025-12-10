use core::{
    cell::UnsafeCell,
    str::Utf8Error,
    sync::atomic::{AtomicBool, Ordering},
    task::{Poll, Waker},
};
use kernel::prelude::*;

pub mod env;
pub mod file;
pub mod namespace;

use arca::{Runtime, Word};
use common::ipaddr::IpAddr;
use common::util::{descriptors::Descriptors, semaphore::Semaphore};
use derive_more::{Display, From, TryInto};
pub use env::Env;
#[cfg(not(feature = "ablation"))]
use lz4_flex::block::compress_prepend_size;
pub use namespace::Namespace;
use vfs::path::Path;

use kernel::{
    kvmclock,
    prelude::RwLock,
    types::{Blob, Function, Tuple, Value},
};
mod table;
use alloc::{
    boxed::Box,
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    sync::Arc,
    vec::Vec,
};
use vfs::{Dir, ErrorKind, File, Object, PathBuf, Result};

use crate::{
    record::{LocalRecord, Record},
    tcpserver,
};

pub struct Proc {
    f: Function,
    pid: u64,
    state: Arc<ProcState>,
    handler: Option<Arc<tcpserver::ClientTx>>,
    img_names_to_contents: Arc<BTreeMap<String, Table>>,
}

impl Proc {
    pub fn new(
        elf: &[u8],
        state: ProcState,
        handler: Arc<tcpserver::ClientTx>,
        img_names_to_contents: Arc<BTreeMap<String, Table>>,
    ) -> core::result::Result<Self, common::elfloader::Error> {
        let f = common::elfloader::load_elf(elf)?;
        let state = Arc::new(state);
        let pid = table::PROCS.allocate(&state);
        let p = Proc {
            f,
            pid,
            state,
            handler: Some(handler),
            img_names_to_contents,
        };
        Ok(p)
    }

    pub fn from_function(
        function: Function,
        state: ProcState,
        handler: Arc<tcpserver::ClientTx>,
        img_names_to_contents: Arc<BTreeMap<String, Table>>,
    ) -> core::result::Result<Self, common::elfloader::Error> {
        let f = function;
        let state = Arc::new(state);
        let pid = table::PROCS.allocate(&state);
        let p = Proc {
            f,
            pid,
            state,
            handler: Some(handler),
            img_names_to_contents,
        };
        Ok(p)
    }

    pub fn from_remote_function(
        function: Function,
        state: ProcState,
        img_names_to_contents: Arc<BTreeMap<String, Table>>,
    ) -> core::result::Result<Self, common::elfloader::Error> {
        let f = function;
        let state = Arc::new(state);
        let pid = table::PROCS.allocate(&state);
        let p = Proc {
            f,
            pid,
            state,
            handler: None,
            img_names_to_contents,
        };
        Ok(p)
    }

    pub async fn run(self, argv: impl IntoIterator<Item = &str>) -> (u8, Record) {
        let _argv = Tuple::from_iter(argv.into_iter().map(Blob::from).map(Value::Blob));
        self.resume().await
    }

    #[allow(clippy::manual_async_fn)]
    pub fn resume(self) -> impl Future<Output = (u8, Record)> + Send {
        async move {
            let mut f = self.f;
            let mut record = LocalRecord::default().into();
            loop {
                let force_start = kvmclock::time_since_boot();
                let result = f.force();

                let Value::Function(g) = result else {
                    log::error!("proc returned something other than an effect!");
                    return (255, record);
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
                    return (255, record);
                };
                f = match (&*effect, &mut *args) {
                    (
                        b"open",
                        &mut [Value::Blob(ref path), Value::Word(flags), Value::Word(mode)],
                    ) => {
                        // TODO this is not the right place for all this logic
                        // Figure out which machine the file is on
                        // filenames will be of the form "hostname:port/path/to/file"
                        let s = &str::from_utf8(path).unwrap();
                        let path_p = Path::new(s);
                        let (host, filename) = path_p.split();
                        let host_ip: IpAddr = host.try_into().unwrap();
                        if host_ip != *self.state.host {
                            #[cfg(feature = "ablation")]
                            {
                                use core::time::Duration;

                                use alloc::string::ToString;

                                use crate::record::RemoteDataRecord;

                                let tcp_init = kvmclock::time_since_boot();

                                log::debug!("Sending File Request");

                                let f = self
                                    .handler
                                    .as_ref()
                                    .expect("Migrated function should not need more continuation handling")
                                    .request_file(filename.to_string())
                                    .await
                                    .expect("Failed to send File Request");

                                log::debug!("Sent File Request");

                                let data = f.await;

                                log::debug!("Received file of size {} bytes", data.len());

                                // Create the file and return file descriptor
                                let mut new_file = vfs::MemFile::default();
                                new_file.write(data.as_slice()).await.unwrap();
                                new_file.seek(vfs::SeekFrom::Start(0)).await.unwrap();
                                let fd = self
                                    .state
                                    .fds
                                    .lock()
                                    .insert(FileDescriptor::File(new_file.boxed()));

                                let file_req = Ok::<usize, u8>(fd);

                                let tcp_end = kvmclock::time_since_boot();

                                record = RemoteDataRecord {
                                    loading_elf: Duration::from_secs(0),
                                    force: tcp_init - force_start,
                                    remote_data_read: tcp_end - tcp_init,
                                    execution: Duration::from_secs(0),
                                }
                                .into();

                                //log::info!(
                                //    "TIMING:\nnetwork read: {} us",
                                //    (tcp_end - tcp_init).as_micros()
                                //);

                                match file_req {
                                    Ok(result) => k.apply(Value::Word(Word::new(result as u64))),
                                    Err(_) => return (1, record),
                                }
                            }

                            #[cfg(not(feature = "ablation"))]
                            {
                                use core::time::Duration;

                                use crate::record::MigratedRecord;

                                log::debug!("sending continuation to open file remotely");

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
                                let k_msg = compress_prepend_size(&msg);
                                let compress_end = kvmclock::time_since_boot();

                                //let size_bytes = msg.len();
                                //let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

                                let res = self.handler.as_ref().expect("Migrated function should not need more continuation handling").request_to_run(k_msg).await;
                                let sending_end = kvmclock::time_since_boot();

                                record = MigratedRecord {
                                    loading_elf: Duration::from_secs(0),
                                    force: k_create_init - force_start,
                                    creation: serialize_init - k_create_init,
                                    serialization: serialize_end - serialize_init,
                                    compression: compress_end - serialize_end,
                                    sending: sending_end - compress_end,
                                }
                                .into();

                                //log::info!(
                                //    "PERF\ncontinuation creation: {} us\nserialization: {} us\ncompression: {} us\nsize: {} bytes ({:.2} MB)",
                                //    (serialize_init - k_create_init).as_micros(),
                                //    (serialize_end - serialize_init).as_micros(),
                                //    (compress_end - serialize_end).as_micros(),
                                //    size_bytes,
                                //    size_mb
                                //);

                                match res {
                                    Ok(_) => return (0, record),
                                    Err(_) => return (1, record),
                                }
                            }
                        } else {
                            let effect_handling_start = kvmclock::time_since_boot();
                            let file = self
                                .img_names_to_contents
                                .get(&filename.file_name().unwrap().to_string())
                                .expect("File does not exist");
                            //let file = fix(file::open(
                            //    &self.state,
                            //    filename.as_bytes(),
                            //    file::OpenFlags(flags.read() as u32),
                            //    file::ModeT(mode.read() as u32),
                            //)
                            //.await);
                            let effect_handling_done = kvmclock::time_since_boot();
                            match &mut record {
                                Record::LocalRecord(local_record) => {
                                    local_record.force = effect_handling_start - force_start;
                                    local_record.handle_effect =
                                        effect_handling_done - effect_handling_start;
                                }
                                Record::RemoteDataRecord(_)
                                | Record::MigratedRecord(_)
                                | Record::RemoteInvocationRecord(_) => {
                                    panic!("Unexpected record type")
                                }
                            }
                            k.apply(file.clone())
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
                            handler: self.handler.clone(),
                            img_names_to_contents: self.img_names_to_contents.clone(),
                        };
                        kernel::rt::spawn(async move { new.resume().await });
                        k.apply(fix(Ok(pid as u32)))
                    }
                    (b"exit", &mut [Value::Word(result)]) => {
                        let force_end = kvmclock::time_since_boot();
                        match &mut record {
                            Record::LocalRecord(local_record) => {
                                local_record.execution = force_end - force_start;
                            }
                            Record::RemoteDataRecord(remote_data_record) => {
                                remote_data_record.execution = force_end - force_start;
                            }
                            Record::MigratedRecord(_) | Record::RemoteInvocationRecord(_) => {
                                panic!("Unexpected record type")
                            }
                        }
                        return (result.read() as u8, record);
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
