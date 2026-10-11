use crate::doorbell::{HostToVMDoorBell, VMToHostDoorBell, new_vm_to_host_door_bell};
use crate::pipe::{ControlPipe, FilePipe, GuestPipe, ListenerPipe, StreamPipe};
use common::BuddyAllocator;
use common::protocol::control::PipeData;
use kvm_ioctls::VmFd;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;

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
) -> (common::pipe::Pipe<VMToHostDoorBell>, GuestPipe) {
    let pipe_idx = next_pipe_idx.fetch_add(1, std::sync::atomic::Ordering::Release);
    let (rx_avail_vm, rx_avail_waiter) = new_vm_to_host_door_bell(vm, pipe_idx as u64 * 2);
    let (tx_avail_vm, tx_avail_waiter) = new_vm_to_host_door_bell(vm, pipe_idx as u64 * 2 + 1);

    let rx_avail_host = HostToVMDoorBell::new(vm);
    let tx_avail_host = HostToVMDoorBell::new(vm);

    let (p0, p1) = common::pipe::pipe(len, rx_avail_vm, tx_avail_vm, rx_avail_host, tx_avail_host);
    let p1 = GuestPipe::new(p1, rx_avail_waiter, tx_avail_waiter);
    (p0, p1)
}

pub fn control_thread(
    vm: Arc<VmFd>,
    next_pipe_idx: Arc<AtomicUsize>,
    argv: Vec<String>,
    mut pipe: ControlPipe,
) {
    use common::protocol::control::*;
    loop {
        let response = match pipe.recv() {
            Request::GetArgs => Response::Args(argv.clone()),
            Request::Exit(code) => std::process::exit(code),
            Request::Open(path, mode) => {
                let f = open_file(&path, mode);
                file_response(f, &vm, &next_pipe_idx)
            }
            Request::Exists(path) => match std::fs::exists(path) {
                Ok(exists) => Response::Bool(exists),
                Err(error) => Response::Err(error.kind().into()),
            },
            Request::ReadFile(path, mut buffer) => match unsafe { read_file(&path, &mut buffer) } {
                Ok(length) => Response::Length(length),
                Err(error) => Response::Err(error.kind().into()),
            },
            Request::WriteFile(path, buffer, mode) => {
                let create_new = mode.create_new;
                io_response(open_file(&path, mode).and_then(|mut file| {
                    let result = file.write_all(unsafe { buffer.as_slice() });
                    if create_new && result.is_err() {
                        let _ = std::fs::remove_file(&path);
                    }
                    result
                }))
            }
            Request::Stdio(stream) => {
                let fd = match stream {
                    StandardStream::Stdin => std::io::stdin().as_fd().try_clone_to_owned(),
                    StandardStream::Stdout => std::io::stdout().as_fd().try_clone_to_owned(),
                    StandardStream::Stderr => std::io::stderr().as_fd().try_clone_to_owned(),
                };
                file_response(fd.map(File::from), &vm, &next_pipe_idx)
            }
            Request::IsDirectory(path) => match std::fs::metadata(path) {
                Ok(metadata) => Response::Bool(metadata.is_dir()),
                Err(e) => Response::Err(e.kind().into()),
            },
            Request::ReadDirectory(path) => {
                let names = std::fs::read_dir(path).and_then(|entries| {
                    entries
                        .map(|entry| {
                            entry?
                                .file_name()
                                .into_string()
                                .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))
                        })
                        .collect::<std::io::Result<Vec<_>>>()
                });
                match names {
                    Ok(names) => Response::Names(names),
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
            Request::Canonicalize(path) => match std::fs::canonicalize(path) {
                Ok(path) => match path.into_os_string().into_string() {
                    Ok(path) => Response::Path(path),
                    Err(_) => Response::Err(IoErrorKind::InvalidData),
                },
                Err(e) => Response::Err(e.kind().into()),
            },
            Request::SetCurrentDirectory(path) => io_response(std::env::set_current_dir(path)),
            Request::Rename(from, to) => io_response(std::fs::rename(from, to)),
            Request::Symlink(target, path) => io_response(std::os::unix::fs::symlink(target, path)),
            Request::RemoveFile(path) => io_response(std::fs::remove_file(path)),
            Request::ReadLink(path) => match std::fs::read_link(path) {
                Ok(target) => match target.into_os_string().into_string() {
                    Ok(target) => Response::Path(target),
                    Err(_) => Response::Err(IoErrorKind::InvalidData),
                },
                Err(e) => Response::Err(e.kind().into()),
            },
            Request::Mkdir(path) => match std::fs::create_dir_all(&path) {
                Ok(()) => Response::Ack,
                Err(e) => Response::Err(e.kind().into()),
            },
            Request::Listen { ip, port } => {
                let listener = TcpListener::bind(SocketAddr::from((ip, port))).unwrap();
                let (p, q) = new_pipe(&vm, 1024, &next_pipe_idx);
                let vm_cl = vm.clone();
                let next_pipe_cl = next_pipe_idx.clone();
                std::thread::spawn(move || {
                    listener_thread(vm_cl, next_pipe_cl, listener, ListenerPipe::new(q));
                });
                Response::Pipe(decompose_pipe(p))
            }
            Request::Connect { host, port } => {
                let stream = TcpStream::connect((host.as_str(), port)).unwrap();
                let (p, q) = new_pipe(&vm, 1024, &next_pipe_idx);
                std::thread::spawn(move || {
                    stream_thread(stream, StreamPipe::new(q));
                });
                Response::Pipe(decompose_pipe(p))
            }
        };
        pipe.send(&response);
    }
}

fn open_file(path: &str, mode: common::protocol::control::FileMode) -> std::io::Result<File> {
    OpenOptions::new()
        .read(mode.read)
        .write(mode.write)
        .create(mode.create)
        .create_new(mode.create_new)
        .append(mode.append)
        .truncate(mode.truncate)
        .open(path)
}

/// # Safety
/// The peer must keep the buffer initialized and exclusively borrowed until the response.
unsafe fn read_file(path: &str, buffer: &mut common::buffer::Region) -> std::io::Result<usize> {
    let mut file = File::open(path)?;
    let bytes = unsafe { buffer.as_mut_slice() };
    let length = usize::try_from(file.metadata()?.len())
        .map_err(|_| std::io::Error::from(std::io::ErrorKind::OutOfMemory))?;
    if length > bytes.len() {
        return Ok(length);
    }
    let mut length = 0;
    while length < bytes.len() {
        match file.read(&mut bytes[length..]) {
            Ok(0) => return Ok(length),
            Ok(read) => length += read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    let mut extra = [0];
    loop {
        match file.read(&mut extra) {
            Ok(read) => return Ok(length + read),
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
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
) -> common::protocol::control::Response {
    use common::protocol::control::Response;
    match file {
        Ok(file) => {
            let (p, q) = new_pipe(vm, 1024, next_pipe_idx);
            std::thread::spawn(move || unsafe { file_thread(file, FilePipe::new(q)) });
            Response::Pipe(decompose_pipe(p))
        }
        Err(e) => Response::Err(e.kind().into()),
    }
}

/// # Safety
/// The peer must hold each region alive until its response, exclusively for reads
/// and immutably for writes.
pub unsafe fn file_thread(mut file: File, mut pipe: FilePipe) {
    use common::protocol::file::*;
    loop {
        let response = match pipe.recv() {
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
                pipe.send(&Response::Ack);
                return;
            }
        };
        pipe.send(&response);
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
        assert!(encoded.len() < 32);
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
        assert_eq!(unsafe { region.as_slice() }.as_ptr(), shared.as_ptr());
        drop(request);
        assert_eq!(
            RefCnt::refcnt(&shared).load(std::sync::atomic::Ordering::SeqCst),
            1
        );

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
    listener: TcpListener,
    mut pipe: ListenerPipe,
) {
    use common::protocol::listener::*;
    loop {
        let response = match pipe.recv() {
            Request::Accept => {
                let (stream, _) = listener.accept().unwrap();
                let (p, q) = new_pipe(&vm, 1024, &next_pipe_idx);
                std::thread::spawn(move || {
                    stream_thread(stream, StreamPipe::new(q));
                });
                Response::Pipe(decompose_pipe(p))
            }
            Request::Close => {
                pipe.send(&Response::Ack);
                return;
            }
        };
        pipe.send(&response);
    }
}

pub fn stream_thread(mut stream: TcpStream, mut pipe: StreamPipe) {
    use common::protocol::stream::*;
    loop {
        let response = match pipe.recv() {
            Request::Receive(len) => {
                let mut buf = vec![0; len];
                let len = stream.read(&mut buf).unwrap();
                buf.truncate(len);
                Response::Bytes(buf)
            }
            Request::Send(bytes) => {
                let len = stream.write(&bytes).unwrap();
                Response::Length(len)
            }
            Request::Close => {
                pipe.send(&Response::Ack);
                return;
            }
        };
        pipe.send(&response);
    }
}
