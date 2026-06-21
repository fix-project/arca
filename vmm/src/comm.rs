use crate::pipe::GuestPipe;
use common::protocol::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::net::{TcpStream, TcpListener, SocketAddr};

pub fn communication_thread(argv: Vec<String>, mut pipe: GuestPipe) {
    loop {
        let mut length = [0; 8];
        pipe.read_exact(&mut length).unwrap();
        let length = usize::from_le_bytes(length);
        let mut bytes = vec![0; length];
        pipe.read_exact(&mut bytes).unwrap();
        let response: Result<Response, Error> = match postcard::from_bytes(&bytes).unwrap() {
            Request::GetArgs => {
                Ok(Response::Args(argv.clone()))
            }
            Request::Exit(code) => {
                std::process::exit(code)
            }
            Request::Open(path, mode) => {
                let f = OpenOptions::new()
                    .read(mode.read)
                    .write(mode.write)
                    .create(mode.create)
                    .append(mode.append)
                    .truncate(mode.truncate)
                    .open(path)
                    .map(Box::new)
                    .map(Box::into_raw);
                match f {
                    Ok(p) => Ok(Response::File(FileDescriptor(p as usize))),
                    Err(x) => todo!("open: {x:?}")
                }
            }
            Request::Close(FileDescriptor(p)) => {
                unsafe {
                    let _ = Box::from_raw(p as *mut File);
                }
                Ok(Response::Ack)
            }
            Request::Read(FileDescriptor(p), len) => {
                let f = unsafe { &mut *(p as *mut File) };
                let mut buf = vec![0; len];
                let len = f.read(&mut buf).unwrap();
                buf.truncate(len);
                Ok(Response::Bytes(buf))
            }
            Request::Write(FileDescriptor(p), bytes) => {
                let f = unsafe { &mut *(p as *mut File) };
                let len = f.write(&bytes).unwrap();
                Ok(Response::Length(len))
            }
            Request::Seek(FileDescriptor(p), whence) => {
                let f = unsafe { &mut *(p as *mut File) };
                let from = match whence {
                    Whence::Start(x) => SeekFrom::Start(x),
                    Whence::Current(x) => SeekFrom::Current(x),
                    Whence::End(x) => SeekFrom::End(x),
                };
                let offset = f.seek(from).unwrap();
                Ok(Response::Offset(offset))
            }
            Request::Listen { ip, port } => {
                let listener = Box::into_raw(Box::new(TcpListener::bind(SocketAddr::from((ip, port))).unwrap()));
                Ok(Response::Listener(ListenerDescriptor(listener as usize)))
            }
            Request::Accept(ListenerDescriptor(l)) => {
                let listener = unsafe { &mut *(l as *mut TcpListener) };
                let (stream, _) = listener.accept().unwrap();
                let stream = Box::into_raw(Box::new(stream));
                Ok(Response::Stream(StreamDescriptor(stream as usize)))
            }
            Request::StopListening(ListenerDescriptor(p)) => {
                unsafe {
                    let _ = Box::from_raw(p as *mut TcpListener);
                }
                Ok(Response::Ack)
            }
            Request::Connect { host, port } => {
                let stream = TcpStream::connect((host.as_str(), port));
                let stream = Box::into_raw(Box::new(stream));
                Ok(Response::Stream(StreamDescriptor(stream as usize)))
            }
            Request::Disconnect(StreamDescriptor(p)) => {
                unsafe {
                    let _ = Box::from_raw(p as *mut TcpStream);
                }
                Ok(Response::Ack)
            }
            Request::Receive(StreamDescriptor(p), len) => {
                let f = unsafe { &mut *(p as *mut TcpStream) };
                let mut buf = vec![0; len];
                let len = f.read(&mut buf).unwrap();
                buf.truncate(len);
                Ok(Response::Bytes(buf))
            }
            Request::Send(StreamDescriptor(p), bytes) => {
                let f = unsafe { &mut *(p as *mut TcpStream) };
                let len = f.write(&bytes).unwrap();
                Ok(Response::Length(len))
            }
        };
        // TODO: with the right trait impls, we could avoid allocating here
        let bytes = postcard::to_allocvec(&response).unwrap();
        let length = bytes.len().to_le_bytes();
        pipe.write_exact(&length).unwrap();
        pipe.write_exact(&bytes).unwrap();
    }
}
