use crate::doorbell::{HostToVMDoorBell, VMToHostDoorBell, new_vm_to_host_door_bell};
use crate::pipe::{FilePipe, GuestPipe, ListenerPipe, StreamPipe};
use crate::session::Session;
use common::BuddyAllocator;
use common::protocol::control::PipeData;
use kvm_ioctls::VmFd;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::fd::{AsFd, AsRawFd};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, RwLock};

fn decompose_pipe(pipe: common::pipe::Pipe<VMToHostDoorBell>) -> PipeData {
    let (rx, tx, rx_avail, tx_avail) = pipe.into_inner();
    let rx = rx.into_inner();
    let tx = tx.into_inner();
    let (rx_ptr, rx_len) = Arc::into_raw_with_allocator(rx).0.to_raw_parts();
    let rx_ptr = BuddyAllocator.to_offset(rx_ptr);
    let (tx_ptr, tx_len) = Arc::into_raw_with_allocator(tx).0.to_raw_parts();
    let tx_ptr = BuddyAllocator.to_offset(tx_ptr);
    PipeData {
        rx_ptr,
        rx_len,
        tx_ptr,
        tx_len,
        rx_avail: rx_avail.into_raw_parts(),
        tx_avail: tx_avail.into_raw_parts(),
    }
}

pub fn new_pipe(
    vm: &VmFd,
    len: usize,
    next_pipe_idx: &Arc<AtomicUsize>,
    doorbell: &HostToVMDoorBell,
) -> (common::pipe::Pipe<VMToHostDoorBell>, GuestPipe) {
    let pipe_idx = next_pipe_idx.fetch_add(1, std::sync::atomic::Ordering::Release);
    let (rx_avail_vm, rx_avail_waiter) = new_vm_to_host_door_bell(vm, pipe_idx as u64 * 2);
    let (tx_avail_vm, tx_avail_waiter) = new_vm_to_host_door_bell(vm, pipe_idx as u64 * 2 + 1);

    let rx_avail_host = doorbell.clone();
    let tx_avail_host = doorbell.clone();

    let (p0, p1) = common::pipe::pipe(len, rx_avail_vm, tx_avail_vm, rx_avail_host, tx_avail_host);
    let p1 =
        GuestPipe::new(p1, rx_avail_waiter, tx_avail_waiter).with_session(doorbell.session.clone());
    (p0, p1)
}

/// State shared by the threads that answer control requests.
struct Control {
    vm: Arc<VmFd>,
    next_pipe_idx: Arc<AtomicUsize>,
    doorbell: HostToVMDoorBell,
    argv: Vec<String>,
    session: Arc<Session>,
    directory: RwLock<PathBuf>,
}

/// Answers control requests concurrently; each response carries its request's id.
pub fn control_thread(
    vm: Arc<VmFd>,
    next_pipe_idx: Arc<AtomicUsize>,
    doorbell: HostToVMDoorBell,
    argv: Vec<String>,
    pipe: GuestPipe,
    session: Arc<Session>,
    directory: PathBuf,
) {
    use common::protocol::control::*;
    let (mut reader, writer) = pipe.split();
    let writer = Arc::new(Mutex::new(writer));
    let control = Arc::new(Control {
        vm,
        next_pipe_idx,
        doorbell,
        argv,
        session: session.clone(),
        directory: RwLock::new(directory),
    });
    let (requests, queue) = std::sync::mpsc::channel::<Tagged<Request>>();
    let queue = Arc::new(Mutex::new(queue));
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    for _ in 0..threads {
        let (control, writer, queue) = (control.clone(), writer.clone(), queue.clone());
        session.spawn(move || {
            loop {
                let Ok(request) = queue.lock().unwrap().recv() else {
                    return;
                };
                if control.session.stopped() {
                    return;
                }
                let message = control.respond(request.message);
                let response = Tagged {
                    id: request.id,
                    message,
                };
                if writer.lock().unwrap().send(&response).is_err() {
                    return;
                }
            }
        });
    }
    loop {
        let Ok(request) = reader.recv::<Tagged<Request>>() else {
            return;
        };
        if requests.send(request).is_err() {
            return;
        }
    }
}

impl Control {
    fn respond(
        &self,
        request: common::protocol::control::Request,
    ) -> common::protocol::control::Response {
        use common::protocol::control::*;
        match request {
            Request::GetArgs => Response::Args(self.argv.clone()),
            Request::Exit(code) => {
                self.session.stop(code as u8);
                Response::Ack
            }
            Request::Open(path, mode) => {
                let f = open_file(self.directory.read().unwrap().join(&path), mode);
                file_response(f, &self.vm, &self.next_pipe_idx, &self.doorbell)
            }
            Request::Exists(path) => {
                match std::fs::exists(self.directory.read().unwrap().join(path)) {
                    Ok(exists) => Response::Bool(exists),
                    Err(error) => Response::Err(error.kind().into()),
                }
            }
            Request::ReadFile(path) => match read_file(self.directory.read().unwrap().join(path)) {
                Ok(bytes) => Response::Bytes(common::buffer::Region::from_owned(bytes)),
                Err(error) => Response::Err(error.kind().into()),
            },
            Request::WriteFile(path, buffer, mode) => {
                let path = self.directory.read().unwrap().join(path);
                let create_new = mode.create_new;
                io_response(open_file(&path, mode).and_then(|mut file| {
                    let result = file.write_all(unsafe { buffer.as_slice() });
                    if create_new && result.is_err() {
                        let _ = std::fs::remove_file(&path);
                    }
                    result
                }))
            }
            Request::Stdio(StandardStream::Stdout) if self.session.captures_output() => {
                let (p, q) = new_pipe(&self.vm, 1024, &self.next_pipe_idx, &self.doorbell);
                let output = CapturedOutput(self.session.clone());
                self.session
                    .spawn(move || unsafe { file_thread(output, FilePipe::new(q)) });
                Response::Pipe(decompose_pipe(p))
            }
            Request::Stdio(stream) => {
                let fd = match stream {
                    StandardStream::Stdin | StandardStream::Stdout if self.session.has_stdio() => {
                        self.session.stdio().unwrap()
                    }
                    StandardStream::Stdin => std::io::stdin().as_fd().try_clone_to_owned(),
                    StandardStream::Stdout => std::io::stdout().as_fd().try_clone_to_owned(),
                    StandardStream::Stderr => std::io::stderr().as_fd().try_clone_to_owned(),
                };
                file_response(
                    fd.map(File::from),
                    &self.vm,
                    &self.next_pipe_idx,
                    &self.doorbell,
                )
            }
            Request::IsDirectory(path) => {
                match std::fs::metadata(self.directory.read().unwrap().join(path)) {
                    Ok(metadata) => Response::Bool(metadata.is_dir()),
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::ReadDirectory(path) => {
                let names = std::fs::read_dir(self.directory.read().unwrap().join(path)).and_then(
                    |entries| {
                        entries
                            .map(|entry| {
                                entry?.file_name().into_string().map_err(|_| {
                                    std::io::Error::from(std::io::ErrorKind::InvalidData)
                                })
                            })
                            .collect::<std::io::Result<Vec<_>>>()
                    },
                );
                match names {
                    Ok(names) => Response::Names(names),
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::Canonicalize(path) => {
                match std::fs::canonicalize(self.directory.read().unwrap().join(path)) {
                    Ok(path) => match path.into_os_string().into_string() {
                        Ok(path) => Response::Path(path),
                        Err(_) => Response::Err(IoErrorKind::InvalidData),
                    },
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::SetCurrentDirectory(path) => io_response((|| {
                let path = std::fs::canonicalize(self.directory.read().unwrap().join(path))?;
                if !path.is_dir() {
                    return Err(std::io::ErrorKind::NotADirectory.into());
                }
                *self.directory.write().unwrap() = path;
                Ok(())
            })()),
            Request::Rename(from, to) => io_response(std::fs::rename(
                self.directory.read().unwrap().join(from),
                self.directory.read().unwrap().join(to),
            )),
            Request::Symlink(target, path) => io_response(std::os::unix::fs::symlink(
                target,
                self.directory.read().unwrap().join(path),
            )),
            Request::RemoveFile(path) => io_response(std::fs::remove_file(
                self.directory.read().unwrap().join(path),
            )),
            Request::ReadLink(path) => {
                match std::fs::read_link(self.directory.read().unwrap().join(path)) {
                    Ok(target) => match target.into_os_string().into_string() {
                        Ok(target) => Response::Path(target),
                        Err(_) => Response::Err(IoErrorKind::InvalidData),
                    },
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::Mkdir(path) => {
                match std::fs::create_dir_all(self.directory.read().unwrap().join(path)) {
                    Ok(()) => Response::Ack,
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::Listen { ip, port } => {
                let listener = TcpListener::bind(SocketAddr::from((ip, port))).unwrap();
                let (p, q) = new_pipe(&self.vm, 1024, &self.next_pipe_idx, &self.doorbell);
                let vm_cl = self.vm.clone();
                let next_pipe_cl = self.next_pipe_idx.clone();
                let doorbell = self.doorbell.clone();
                self.session.spawn(move || {
                    listener_thread(
                        vm_cl,
                        next_pipe_cl,
                        doorbell,
                        listener,
                        ListenerPipe::new(q),
                    );
                });
                Response::Pipe(decompose_pipe(p))
            }
            Request::OpenUri(uri) => match open_uri(&uri) {
                Ok(body) => {
                    let (p, q) = new_pipe(&self.vm, 1024, &self.next_pipe_idx, &self.doorbell);
                    self.session
                        .spawn(move || stream_thread(ReadOnly(body), StreamPipe::new(q)));
                    Response::Pipe(decompose_pipe(p))
                }
                Err(error) => Response::Err(error.kind().into()),
            },
            Request::Connect { host, port } => match TcpStream::connect((host.as_str(), port)) {
                Ok(stream) => {
                    let (p, q) = new_pipe(&self.vm, 1024, &self.next_pipe_idx, &self.doorbell);
                    let stream = Cancellable {
                        inner: stream,
                        session: self.session.clone(),
                    };
                    self.session
                        .spawn(move || stream_thread(stream, StreamPipe::new(q)));
                    Response::Pipe(decompose_pipe(p))
                }
                Err(error) => Response::Err(error.kind().into()),
            },
        }
    }
}

fn open_file(
    path: impl AsRef<Path>,
    mode: common::protocol::control::FileMode,
) -> std::io::Result<File> {
    OpenOptions::new()
        .read(mode.read)
        .write(mode.write)
        .create(mode.create)
        .create_new(mode.create_new)
        .append(mode.append)
        .truncate(mode.truncate)
        .open(path)
}

fn read_file(path: impl AsRef<Path>) -> std::io::Result<Box<[u8], BuddyAllocator>> {
    let mut file = File::open(path)?;
    let length = usize::try_from(file.metadata()?.len())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
    let mut bytes = Vec::new_in(BuddyAllocator);
    bytes
        .try_reserve_exact(length)
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
    loop {
        if bytes.len() == bytes.capacity() {
            let mut next = [0];
            match file.read(&mut next) {
                Ok(0) => return Ok(bytes.into_boxed_slice()),
                Ok(_) => {
                    bytes
                        .try_reserve(bytes.capacity().max(4096))
                        .map_err(|_| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
                    bytes.push(next[0]);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        let mut spare = std::io::BorrowedBuf::from(bytes.spare_capacity_mut());
        match file.read_buf(spare.unfilled()) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
        let read = spare.len();
        if read == 0 {
            return Ok(bytes.into_boxed_slice());
        }
        unsafe { bytes.set_len(bytes.len() + read) };
    }
}

fn io_response(result: std::io::Result<()>) -> common::protocol::control::Response {
    use common::protocol::control::Response;
    match result {
        Ok(()) => Response::Ack,
        Err(e) => Response::Err(e.kind().into()),
    }
}

fn file_response(
    file: std::io::Result<File>,
    vm: &Arc<VmFd>,
    next_pipe_idx: &Arc<AtomicUsize>,
    doorbell: &HostToVMDoorBell,
) -> common::protocol::control::Response {
    use common::protocol::control::Response;
    match file {
        Ok(file) => {
            let (p, q) = new_pipe(vm, 1024, next_pipe_idx, doorbell);
            let session = doorbell.session.as_ref().unwrap();
            let file = Cancellable {
                inner: file,
                session: session.clone(),
            };
            session.spawn(move || unsafe { file_thread(file, FilePipe::new(q)) });
            Response::Pipe(decompose_pipe(p))
        }
        Err(e) => Response::Err(e.kind().into()),
    }
}

/// # Safety
/// The peer must hold each region alive until its response, exclusively for reads
/// and immutably for writes.
pub unsafe fn file_thread(mut file: impl Read + Write + Seek, mut pipe: FilePipe) {
    use common::protocol::file::*;
    loop {
        let Ok(request) = pipe.recv() else {
            return;
        };
        let response = match request {
            Request::Read(mut buffer) => match file.read(unsafe { buffer.as_mut_slice() }) {
                Ok(len) => Response::Length(len),
                Err(e) => Response::Err(e.kind().into()),
            },
            Request::Write(bytes) => match file.write(unsafe { bytes.as_slice() }) {
                Ok(len) => Response::Length(len),
                Err(e) => Response::Err(e.kind().into()),
            },
            Request::Seek(whence) => {
                let from = match whence {
                    Whence::Start(x) => SeekFrom::Start(x),
                    Whence::Current(x) => SeekFrom::Current(x),
                    Whence::End(x) => SeekFrom::End(x),
                };
                match file.seek(from) {
                    Ok(offset) => Response::Offset(offset),
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::Close => {
                let _ = pipe.send(&Response::Ack);
                return;
            }
        };
        if pipe.send(&response).is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use common::{
        BuddyAllocator,
        buffer::{Buffer, Region},
        protocol::file::Request,
        refcnt::RefCnt,
    };

    #[test]
    fn buffers_lend_unique_and_shared_storage() {
        let mut unique = Buffer::from(Box::new_in([0u8; 4096], BuddyAllocator));
        let request = Request::Read(unique.slice_mut(..));
        let encoded = postcard::to_allocvec(&request).unwrap();
        let Request::Read(mut region) = postcard::from_bytes::<Request>(&encoded).unwrap() else {
            panic!("expected read");
        };
        unsafe { region.as_mut_slice() }.fill(0xa5);
        assert_eq!(*unique, [0xa5; 4096]);

        let shared: RefCnt<_> = unique.into_inner().into();
        let request = Request::Write(Buffer::from(shared.clone()));
        let encoded = postcard::to_allocvec(&request).unwrap();
        let Request::Write(region) = postcard::from_bytes::<Request>(&encoded).unwrap() else {
            panic!("expected write");
        };
        assert_eq!(unsafe { region.as_slice() }, &*shared);
        drop(request);

        let shared = Buffer::from(shared);
        let empty = postcard::to_allocvec(&shared.slice(..0)).unwrap();
        let region: Region = postcard::from_bytes(&empty).unwrap();
        assert!(unsafe { region.as_slice() }.is_empty());
    }

    #[test]
    fn buffer_ranges_are_checked_before_access() {
        for (offset, len) in [(BuddyAllocator.len(), 1), (usize::MAX, 2)] {
            let encoded = postcard::to_allocvec(&(offset, len)).unwrap();
            let region: Region = postcard::from_bytes(&encoded).unwrap();
            assert!(std::panic::catch_unwind(|| unsafe { region.as_slice() }).is_err());
        }
    }
}

pub fn listener_thread(
    vm: Arc<VmFd>,
    next_pipe_idx: Arc<AtomicUsize>,
    doorbell: HostToVMDoorBell,
    listener: TcpListener,
    mut pipe: ListenerPipe,
) {
    use common::protocol::listener::*;
    loop {
        let Ok(request) = pipe.recv() else {
            return;
        };
        let response = match request {
            Request::Accept => {
                let session = doorbell.session.as_ref().unwrap();
                if session.wait(listener.as_raw_fd(), libc::POLLIN).is_err() {
                    return;
                }
                let (stream, _) = listener.accept().unwrap();
                let (p, q) = new_pipe(&vm, 1024, &next_pipe_idx, &doorbell);
                let stream = Cancellable {
                    inner: stream,
                    session: session.clone(),
                };
                session.spawn(move || {
                    stream_thread(stream, StreamPipe::new(q));
                });
                Response::Pipe(decompose_pipe(p))
            }
            Request::Close => {
                let _ = pipe.send(&Response::Ack);
                return;
            }
        };
        if pipe.send(&response).is_err() {
            return;
        }
    }
}

fn open_uri(uri: &str) -> std::io::Result<Box<dyn Read + Send>> {
    if let Some((scheme, path)) = uri.split_once(':')
        && scheme.eq_ignore_ascii_case("file")
    {
        return Ok(Box::new(File::open(file_uri_path(path)?)?));
    }
    let parsed: ureq::http::Uri = uri.parse().map_err(|_| std::io::ErrorKind::InvalidInput)?;
    if !matches!(parsed.scheme_str(), Some("http" | "https")) {
        return Err(std::io::ErrorKind::Unsupported.into());
    }
    let response = ureq::get(uri).call().map_err(std::io::Error::other)?;
    Ok(Box::new(response.into_body().into_reader()))
}

fn file_uri_path(path: &str) -> std::io::Result<std::path::PathBuf> {
    use std::io::ErrorKind::{InvalidInput, Unsupported};
    use std::os::unix::ffi::OsStringExt;

    let path = if let Some(authority) = path.strip_prefix("//") {
        let (host, _) = authority.split_once('/').ok_or(InvalidInput)?;
        if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
            return Err(Unsupported.into());
        }
        &authority[host.len()..]
    } else {
        path
    };
    if !path.starts_with('/') || path.contains(['?', '#']) {
        return Err(InvalidInput.into());
    }
    let mut bytes = path.bytes();
    let mut decoded = Vec::with_capacity(path.len());
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = char::from(bytes.next().ok_or(InvalidInput)?)
                .to_digit(16)
                .ok_or(InvalidInput)?;
            let low = char::from(bytes.next().ok_or(InvalidInput)?)
                .to_digit(16)
                .ok_or(InvalidInput)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    Ok(std::ffi::OsString::from_vec(decoded).into())
}

struct ReadOnly<R>(R);
impl<R: Read> Read for ReadOnly<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(bytes)
    }
}
impl<R> Write for ReadOnly<R> {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn stream_response(
    stream: &mut (impl Read + Write),
    request: common::protocol::stream::Request,
) -> common::protocol::stream::Response {
    use common::protocol::stream::*;
    match request {
        Request::Receive(len) => {
            let mut buf = vec![0; len];
            match stream.read(&mut buf) {
                Ok(len) => {
                    buf.truncate(len);
                    Response::Bytes(buf)
                }
                Err(error) => Response::Err(error.kind().into()),
            }
        }
        Request::Send(bytes) => match stream.write(&bytes) {
            Ok(len) => Response::Length(len),
            Err(error) => Response::Err(error.kind().into()),
        },
        Request::Close => Response::Ack,
    }
}

pub fn stream_thread(mut stream: impl Read + Write, mut pipe: StreamPipe) {
    use common::protocol::stream::*;
    loop {
        let Ok(request) = pipe.recv() else {
            return;
        };
        let close = matches!(request, Request::Close);
        if pipe.send(&stream_response(&mut stream, request)).is_err() {
            return;
        }
        if close {
            return;
        }
    }
}

#[cfg(test)]
mod stream_tests {
    use super::stream_response;
    use common::protocol::stream::{Request, Response};
    use std::io::{Cursor, Read, Write};

    #[test]
    fn short_receive_and_eof() {
        let mut stream = Cursor::new(b"hello".to_vec());
        let Response::Bytes(bytes) = stream_response(&mut stream, Request::Receive(65536)) else {
            panic!("receive failed");
        };
        assert_eq!(bytes, b"hello");
        let Response::Bytes(bytes) = stream_response(&mut stream, Request::Receive(65536)) else {
            panic!("EOF failed");
        };
        assert!(bytes.is_empty());
    }

    #[test]
    fn uri_stream_decodes_http_and_reports_eof() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                stream.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            stream.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n").unwrap();
        });
        let mut stream =
            super::ReadOnly(super::open_uri(&format!("http://{address}/source")).unwrap());
        let mut body = Vec::new();
        stream.read_to_end(&mut body).unwrap();
        assert_eq!(body, b"hello world");
        assert!(
            matches!(stream_response(&mut stream, Request::Receive(16)), Response::Bytes(bytes) if bytes.is_empty())
        );
        assert!(matches!(
            stream_response(&mut stream, Request::Send(vec![1])),
            Response::Err(_)
        ));
        server.join().unwrap();
        assert_eq!(
            super::open_uri("ftp://example.invalid/source")
                .err()
                .unwrap()
                .kind(),
            std::io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn file_uri_stream_reads_encoded_local_paths() {
        let directory = std::env::var_os("TEST_TMPDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!("file-uri-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("source #%?.bin");
        let contents = [0, 255, 1, 2, 3];
        std::fs::write(&path, contents).unwrap();
        let encoded = path
            .to_str()
            .unwrap()
            .replace('%', "%25")
            .replace(' ', "%20")
            .replace('#', "%23")
            .replace('?', "%3F");
        for uri in [
            format!("file://{encoded}"),
            format!("file://localhost{encoded}"),
            format!("file:{encoded}"),
        ] {
            let mut stream = super::ReadOnly(super::open_uri(&uri).unwrap());
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, contents);
            assert!(
                matches!(stream_response(&mut stream, Request::Receive(16)), Response::Bytes(bytes) if bytes.is_empty())
            );
            assert!(matches!(
                stream_response(&mut stream, Request::Send(vec![1])),
                Response::Err(_)
            ));
        }
        for uri in [
            "file:relative",
            "file:///bad%",
            "file:///bad%zz",
            "file:///path#fragment",
            "file:///path?query",
        ] {
            assert_eq!(
                super::open_uri(uri).err().unwrap().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        assert_eq!(
            super::open_uri("file://remote.invalid/path")
                .err()
                .unwrap()
                .kind(),
            std::io::ErrorKind::Unsupported
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    struct Failed;
    impl Read for Failed {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::ConnectionReset.into())
        }
    }
    impl Write for Failed {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn network_errors_are_responses() {
        assert!(matches!(
            stream_response(&mut Failed, Request::Receive(16)),
            Response::Err(_)
        ));
        assert!(matches!(
            stream_response(&mut Failed, Request::Send(vec![1, 2, 3])),
            Response::Err(_)
        ));
    }
}

struct Cancellable<T> {
    inner: T,
    session: Arc<Session>,
}
impl<T: Read + AsRawFd> Read for Cancellable<T> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.session.wait(self.inner.as_raw_fd(), libc::POLLIN)?;
        self.inner.read(bytes)
    }
}
impl<T: Write + AsRawFd> Write for Cancellable<T> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        self.session.wait(self.inner.as_raw_fd(), libc::POLLOUT)?;
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
impl<T: Seek> Seek for Cancellable<T> {
    fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
        self.inner.seek(from)
    }
}
struct CapturedOutput(Arc<Session>);
impl Write for CapturedOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write_output(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Read for CapturedOutput {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}
impl Seek for CapturedOutput {
    fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
        Err(std::io::ErrorKind::Unsupported.into())
    }
}
