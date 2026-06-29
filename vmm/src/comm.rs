use crate::pipe::{ControlPipe, FilePipe, ListenerPipe, StreamPipe};
use common::protocol::control::PipeData;
use common::BuddyAllocator;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;

fn decompose_pipe(pipe: common::pipe::Pipe) -> PipeData {
    let (rx, tx) = pipe.into_inner();
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
    }
}

pub fn control_thread(argv: Vec<String>, mut pipe: ControlPipe) {
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
                        let (p, q) = common::pipe::pipe(1024);
                        std::thread::spawn(move || {
                            let pipe = crate::pipe::GuestPipe::new(q);
                            file_thread(f, FilePipe::new(pipe));
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
                let (p, q) = common::pipe::pipe(1024);
                std::thread::spawn(move || {
                    let pipe = crate::pipe::GuestPipe::new(q);
                    listener_thread(listener, ListenerPipe::new(pipe));
                });
                Response::Pipe(decompose_pipe(p))
            }
            Request::Connect { host, port } => {
                let stream = TcpStream::connect((host.as_str(), port)).unwrap();
                let (p, q) = common::pipe::pipe(1024);
                std::thread::spawn(move || {
                    let pipe = crate::pipe::GuestPipe::new(q);
                    stream_thread(stream, StreamPipe::new(pipe));
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
                return;
            }
        };
        pipe.send(&response);
    }
}

pub fn listener_thread(listener: TcpListener, mut pipe: ListenerPipe) {
    use common::protocol::listener::*;
    loop {
        let response = match pipe.recv() {
            Request::Accept => {
                let (stream, _) = listener.accept().unwrap();
                let (p, q) = common::pipe::pipe(1024);
                std::thread::spawn(move || {
                    let pipe = crate::pipe::GuestPipe::new(q);
                    stream_thread(stream, StreamPipe::new(pipe));
                });
                Response::Pipe(decompose_pipe(p))
            }
            Request::Close => {
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
                return;
            }
        };
        pipe.send(&response);
    }
}
