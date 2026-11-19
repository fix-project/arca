use crate::session::Session;
use common::protocol::{
    control::{EvaluationSnapshot, EvaluationStep},
    session::{Command as GuestCommand, Id, Request, Response, Settings},
};
use serde::{Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, VecDeque},
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            net::{UnixListener, UnixStream},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{ChildStdin, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub fn identity() -> io::Result<Id> {
    let mut id = [0; 16];
    let mut remaining = id.as_mut_slice();
    while !remaining.is_empty() {
        let count = unsafe { libc::getrandom(remaining.as_mut_ptr().cast(), remaining.len(), 0) };
        if count < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error);
        }
        if count == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        remaining = &mut remaining[count as usize..];
    }
    Ok(id)
}

fn send(stream: &mut impl Write, message: &impl Serialize) -> io::Result<()> {
    let bytes = postcard::to_allocvec(message).map_err(io::Error::other)?;
    let length = u32::try_from(bytes.len()).map_err(io::Error::other)?;
    stream.write_all(&length.to_le_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()
}
fn receive<T: DeserializeOwned>(stream: &mut impl Read) -> io::Result<T> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let mut bytes = vec![0; u32::from_le_bytes(length) as usize];
    stream.read_exact(&mut bytes)?;
    postcard::from_bytes(&bytes).map_err(io::Error::other)
}

fn lock(file: &File, flags: i32) -> io::Result<()> {
    loop {
        if unsafe { libc::flock(file.as_raw_fd(), flags) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
fn directory() -> io::Result<PathBuf> {
    let path = PathBuf::from(format!("/tmp/fix-sessions-{}", unsafe { libc::geteuid() }));
    match std::fs::DirBuilder::new().mode(0o700).create(&path) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error),
    }
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(path)
}

pub struct Client {
    pub settings: Settings,
    pub node_identity: Id,
    connection: Id,
    path: PathBuf,
    stream: UnixStream,
    next: u64,
    diagnostic: common::progress::Stderr,
    key: String,
}
impl Client {
    pub fn open(
        executable: &Path,
        key: &str,
        settings: Settings,
        create: bool,
    ) -> io::Result<Self> {
        if key.len() != 64 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let directory = directory()?;
        let path = directory.join(format!("{key}.sock"));
        let launch = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(directory.join(format!("{key}.launch")))?;
        lock(&launch, libc::LOCK_EX)?;
        let owner_path = directory.join(format!("{key}.lock"));
        let owner = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(owner_path)?;
        loop {
            let (launched, _startup) = match lock(&owner, libc::LOCK_EX | libc::LOCK_NB) {
                Ok(()) if create => (
                    true,
                    Some(Self::launch(executable, &path, &owner, &settings)?),
                ),
                Ok(()) => {
                    return Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        "no running session for this repository",
                    ));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => (false, None),
                Err(error) => return Err(error),
            };
            let opened = (|| {
                let mut stream = UnixStream::connect(&path)?;
                let request = if create {
                    Request::Create(settings.clone())
                } else {
                    Request::Join
                };
                send(&mut stream, &request)?;
                let response = receive(&mut stream)?;
                Ok::<_, io::Error>((stream, response))
            })();
            match opened {
                Ok((
                    stream,
                    Response::Opened {
                        settings,
                        connection,
                        node,
                    },
                )) => {
                    return Ok(Self {
                        settings,
                        connection,
                        node_identity: node,
                        path,
                        stream,
                        next: 0,
                        diagnostic: common::progress::Stderr::default(),
                        key: key.into(),
                    });
                }
                Ok((_, Response::Error(error))) => return Err(io::Error::other(error)),
                Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
                Err(error)
                    if !launched
                        && matches!(
                            error.kind(),
                            io::ErrorKind::ConnectionRefused
                                | io::ErrorKind::ConnectionReset
                                | io::ErrorKind::UnexpectedEof
                        ) =>
                {
                    lock(&owner, libc::LOCK_EX)?
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn launch(
        executable: &Path,
        path: &Path,
        owner: &File,
        settings: &Settings,
    ) -> io::Result<ChildStdin> {
        match std::fs::remove_file(path) {
            Ok(()) => (),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error),
        }
        let listener = UnixListener::bind(path)?;
        let mut command = Command::new(executable);
        command
            .arg("worker")
            .arg(owner.as_raw_fd().to_string())
            .arg(listener.as_raw_fd().to_string());
        let inherited = [owner.as_raw_fd(), listener.as_raw_fd()];
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                for fd in inherited {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let log = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path.with_extension("log"))?;
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(log)
            .spawn()?;
        let mut startup = child.stdin.take().unwrap();
        send(&mut startup, settings)?;
        let ready = receive::<Response>(&mut child.stdout.take().unwrap());
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        match ready? {
            Response::Attached => Ok(startup),
            Response::Error(error) => Err(io::Error::other(error)),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }

    pub fn attach(&self) -> io::Result<UnixStream> {
        let mut stream = UnixStream::connect(&self.path)?;
        send(
            &mut stream,
            &Request::Attach {
                session: self.settings.id,
                connection: self.connection,
            },
        )?;
        match receive(&mut stream)? {
            Response::Attached => Ok(stream),
            Response::Error(error) => Err(io::Error::other(error)),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }

    pub fn execute(&mut self, arguments: &[String], stdio: bool) -> io::Result<(u8, Vec<u8>)> {
        let request = self.next;
        self.next = self
            .next
            .checked_add(1)
            .ok_or(io::ErrorKind::InvalidInput)?;
        send(
            &mut self.stream,
            &Request::Execute {
                request,
                arguments: arguments.to_vec(),
                stdio,
            },
        )?;
        loop {
            match receive(&mut self.stream)? {
                Response::Finished {
                    request: id,
                    status,
                    output,
                } if id == request => return Ok((status, output)),
                Response::Progress(snapshot) => common::progress::evaluation_progress(snapshot),
                Response::Step(step) => common::progress::evaluation_step(step),
                Response::Diagnostic(bytes) => self.diagnostic.write_all(&bytes)?,
                Response::Error(error) => return Err(io::Error::other(error)),
                _ => return Err(io::ErrorKind::InvalidData.into()),
            }
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let mut close = || -> io::Result<()> {
            let directory = directory()?;
            let launch = OpenOptions::new()
                .read(true)
                .write(true)
                .open(directory.join(format!("{}.launch", self.key)))?;
            lock(&launch, libc::LOCK_EX)?;
            send(&mut self.stream, &Request::Close)?;
            if let Response::Closing { last: true } = receive::<Response>(&mut self.stream)? {
                let owner = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(directory.join(format!("{}.lock", self.key)))?;
                lock(&owner, libc::LOCK_EX)?;
            }
            Ok(())
        };
        let _ = close();
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

struct Peer {
    socket: UnixStream,
    outgoing: Mutex<VecDeque<Response>>,
    ready: Condvar,
    closed: AtomicBool,
    observing: AtomicBool,
    stdio: Mutex<Option<u64>>,
}
impl Peer {
    fn send(&self, response: Response) {
        let mut queue = self.outgoing.lock().unwrap();
        if matches!(&response, Response::Progress(_)) {
            queue.retain(|old| !matches!(old, Response::Progress(_)));
        }
        queue.push_back(response);
        self.ready.notify_one();
    }
    fn stop(&self) {
        self.closed.store(true, Ordering::Release);
        let _ = self.socket.shutdown(Shutdown::Both);
        self.ready.notify_all();
    }
}
struct State {
    peers: BTreeMap<Id, Arc<Peer>>,
    commands: VecDeque<GuestCommand>,
    stopped: bool,
    streams: usize,
}
pub(crate) struct Broker {
    pub settings: Settings,
    listener: UnixListener,
    state: Mutex<State>,
    ready: Condvar,
}
impl Broker {
    fn new(settings: Settings, listener: UnixListener) -> Self {
        Self {
            settings,
            listener,
            state: Mutex::new(State {
                peers: BTreeMap::new(),
                commands: VecDeque::new(),
                stopped: false,
                streams: 0,
            }),
            ready: Condvar::new(),
        }
    }
    pub(crate) fn start(self: &Arc<Self>, session: &Arc<Session>) {
        let broker = self.clone();
        let worker = session.clone();
        session.spawn(move || {
            if worker.wait(libc::STDIN_FILENO, libc::POLLIN).is_err() {
                return;
            }
            let empty = {
                let state = broker.state.lock().unwrap();
                state.peers.is_empty() && state.streams == 0
            };
            if empty {
                worker.stop(0);
            }
        });
        let broker = self.clone();
        let worker = session.clone();
        session.spawn(move || {
            loop {
                if worker
                    .wait(broker.listener.as_raw_fd(), libc::POLLIN)
                    .is_err()
                {
                    return;
                }
                let (socket, _) = match broker.listener.accept() {
                    Ok(socket) => socket,
                    Err(_) => return,
                };
                let (broker, session) = (broker.clone(), worker.clone());
                worker.spawn(move || {
                    let _ = broker.connection(socket, &session);
                });
            }
        });
    }
    fn connection(
        self: &Arc<Self>,
        mut socket: UnixStream,
        session: &Arc<Session>,
    ) -> io::Result<()> {
        let cancel = Arc::new(socket.try_clone()?);
        let stopping = Arc::downgrade(&cancel);
        session.on_stop(move || {
            if let Some(socket) = stopping.upgrade() {
                let _ = socket.shutdown(Shutdown::Both);
            }
        });
        let first: Request = receive(&mut socket)?;
        if let Request::Attach {
            session: id,
            connection,
        } = first
        {
            let peer = self.state.lock().unwrap().peers.get(&connection).cloned();
            if id != self.settings.id || peer.is_none() {
                send(
                    &mut socket,
                    &Response::Error("session connection does not exist".into()),
                )?;
                return Ok(());
            }
            let peer = peer.unwrap();
            let mut stdio = peer.stdio.lock().unwrap();
            if stdio.is_some() {
                send(
                    &mut socket,
                    &Response::Error("connection already has a stream".into()),
                )?;
                return Ok(());
            }
            send(&mut socket, &Response::Attached)?;
            let stream =
                crate::stream::Stream::new(socket.try_clone()?, socket, session.clone(), None)?;
            *stdio = Some(session.streams.insert(self.observe_stream(stream, session)));
            return Ok(());
        }
        if self.state.lock().unwrap().stopped {
            return Err(io::ErrorKind::NotConnected.into());
        }
        let id = match &first {
            Request::Create(settings) if settings.repository == self.settings.repository => {
                settings.id
            }
            Request::Join => self.settings.id,
            _ => {
                send(
                    &mut socket,
                    &Response::Error("invalid session request".into()),
                )?;
                return Ok(());
            }
        };
        if id != self.settings.id {
            send(
                &mut socket,
                &Response::Error("repository already belongs to a different session".into()),
            )?;
            return Ok(());
        }
        let connection = identity()?;
        let peer = Arc::new(Peer {
            socket: socket.try_clone()?,
            outgoing: Mutex::new(VecDeque::new()),
            ready: Condvar::new(),
            closed: AtomicBool::new(false),
            observing: AtomicBool::new(false),
            stdio: Mutex::new(None),
        });
        {
            let mut state = self.state.lock().unwrap();
            if state.stopped {
                return Err(io::ErrorKind::NotConnected.into());
            }
            state.peers.insert(connection, peer.clone());
        }
        let result = (|| {
            send(
                &mut socket,
                &Response::Opened {
                    settings: self.settings.clone(),
                    connection,
                    node: session.node_identity,
                },
            )?;
            let writer = peer.clone();
            let mut output = socket.try_clone()?;
            session.spawn(move || {
                loop {
                    let response = {
                        let mut queue = writer.outgoing.lock().unwrap();
                        while queue.is_empty() && !writer.closed.load(Ordering::Acquire) {
                            queue = writer.ready.wait(queue).unwrap();
                        }
                        if writer.closed.load(Ordering::Acquire) {
                            return;
                        }
                        queue.pop_front().unwrap()
                    };
                    if send(&mut output, &response).is_err() {
                        writer.stop();
                        return;
                    }
                }
            });
            loop {
                let request: Request = receive(&mut socket)?;
                if matches!(request, Request::Close) {
                    let last = {
                        let mut state = self.state.lock().unwrap();
                        state.peers.remove(&connection);
                        let last = state.peers.is_empty() && state.streams == 0;
                        if last {
                            state.stopped = true;
                        }
                        last
                    };
                    send(&mut socket, &Response::Closing { last })?;
                    if last {
                        session.stop(0);
                    }
                    return Ok(());
                }
                let Request::Execute {
                    request,
                    arguments,
                    stdio,
                } = request
                else {
                    return Err(io::ErrorKind::InvalidData.into());
                };
                let stdio = if stdio {
                    Some(
                        peer.stdio
                            .lock()
                            .unwrap()
                            .take()
                            .ok_or(io::ErrorKind::NotConnected)?,
                    )
                } else {
                    None
                };
                peer.observing.store(true, Ordering::Release);
                self.state.lock().unwrap().commands.push_back(GuestCommand {
                    connection,
                    request,
                    arguments,
                    stdio,
                });
                self.ready.notify_one();
            }
        })();
        peer.stop();
        if let Some(stream) = peer.stdio.lock().unwrap().take() {
            session.streams.close(stream);
        }
        let empty = {
            let mut state = self.state.lock().unwrap();
            state.peers.remove(&connection);
            state.peers.is_empty() && state.streams == 0
        };
        if empty {
            session.stop(0);
        }
        result
    }
    pub(crate) fn observe_stream(
        self: &Arc<Self>,
        stream: crate::stream::Stream,
        session: &Arc<Session>,
    ) -> crate::stream::Stream {
        self.state.lock().unwrap().streams += 1;
        let broker = Arc::downgrade(self);
        let session = Arc::downgrade(session);
        stream.on_close(move || {
            if let Some(broker) = broker.upgrade() {
                let empty = {
                    let mut state = broker.state.lock().unwrap();
                    state.streams -= 1;
                    state.streams == 0 && state.peers.is_empty()
                };
                if empty && let Some(session) = session.upgrade() {
                    session.stop(0);
                }
            }
        })
    }
    pub(crate) fn next(&self) -> Option<GuestCommand> {
        let mut state = self.state.lock().unwrap();
        loop {
            if let Some(command) = state.commands.pop_front() {
                return Some(command);
            }
            if state.stopped {
                return None;
            }
            state = self.ready.wait(state).unwrap();
        }
    }
    pub(crate) fn reply(&self, connection: Id, request: u64, status: u8, output: Vec<u8>) {
        if let Some(peer) = self.state.lock().unwrap().peers.get(&connection) {
            peer.observing.store(false, Ordering::Release);
            peer.send(Response::Finished {
                request,
                status,
                output,
            });
        }
    }
    pub(crate) fn progress(&self, snapshot: EvaluationSnapshot) {
        for peer in self.state.lock().unwrap().peers.values() {
            if peer.observing.load(Ordering::Acquire) {
                peer.send(Response::Progress(snapshot.clone()));
            }
        }
    }
    pub(crate) fn step(&self, step: EvaluationStep) {
        for peer in self.state.lock().unwrap().peers.values() {
            if peer.observing.load(Ordering::Acquire) {
                peer.send(Response::Step(step.clone()));
            }
        }
    }
    pub(crate) fn diagnostic(&self, bytes: &[u8]) {
        for peer in self.state.lock().unwrap().peers.values() {
            if peer.observing.load(Ordering::Acquire) {
                peer.send(Response::Diagnostic(bytes.to_vec()));
            }
        }
    }
    pub(crate) fn stop(&self) {
        let mut state = self.state.lock().unwrap();
        state.stopped = true;
        for peer in state.peers.values() {
            peer.stop();
        }
        self.ready.notify_all();
    }
}

/// # Safety
/// The session launcher must pass ownership of the lock and listener descriptors.
pub unsafe fn run_owner(kernel: Arc<[u8]>, lock_fd: i32, listen_fd: i32) -> io::Result<u8> {
    if lock_fd < 3 || listen_fd < 3 || lock_fd == listen_fd {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    for fd in [lock_fd, listen_fd] {
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let _owner = unsafe { OwnedFd::from_raw_fd(lock_fd) };
    let listener = unsafe { UnixListener::from_raw_fd(listen_fd) };
    listener.local_addr()?;
    let settings: Settings = receive(&mut std::io::stdin())?;
    let broker = Arc::new(Broker::new(settings.clone(), listener));
    crate::profiling::set_output(settings.profile.as_ref().map(PathBuf::from));
    let mut runtime =
        crate::runtime::Runtime::new(settings.workers.max(1), settings.memory, kernel);
    send(&mut std::io::stdout(), &Response::Attached)?;
    Ok(runtime.run_service(settings, broker))
}
