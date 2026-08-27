use crate::doorbell::{new_vm_to_host_door_bell, HostToVMDoorBell, VMToHostDoorBell};
use crate::pipe::{ControlPipe, FilePipe, GuestPipe, ListenerPipe, StreamPipe};
use common::protocol::control::PipeData;
use common::BuddyAllocator;
use kvm_ioctls::VmFd;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

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
                let f = OpenOptions::new()
                    .read(mode.read)
                    .write(mode.write)
                    .create(mode.create)
                    .append(mode.append)
                    .truncate(mode.truncate)
                    .open(path);
                match f {
                    Ok(f) => {
                        let (p, q) = new_pipe(&vm, 1024, &next_pipe_idx);
                        std::thread::spawn(move || {
                            file_thread(f, FilePipe::new(q));
                        });
                        Response::Pipe(decompose_pipe(p))
                    }
                    Err(e) => Response::Err(e.kind().into()),
                }
            }
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

pub fn file_thread(mut file: File, mut pipe: FilePipe) {
    use common::protocol::file::*;
    loop {
        let response = match pipe.recv() {
            Request::Read(len) => {
                let mut buf = vec![0; len];
                let len = file.read(&mut buf).unwrap();
                buf.truncate(len);
                Response::Bytes(buf)
            }
            Request::Write(bytes) => {
                let len = file.write(&bytes).unwrap();
                Response::Length(len)
            }
            Request::Seek(whence) => {
                let from = match whence {
                    Whence::Start(x) => SeekFrom::Start(x),
                    Whence::Current(x) => SeekFrom::Current(x),
                    Whence::End(x) => SeekFrom::End(x),
                };
                let offset = file.seek(from).unwrap();
                Response::Offset(offset)
            }
            Request::Close => {
                pipe.send(&Response::Ack);
                return;
            }
        };
        pipe.send(&response);
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
