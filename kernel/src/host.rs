use crate::prelude::*;

use alloc::format;
use common::hypercall;

pub struct HostLogger;

impl log::Log for HostLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            crate::interrupts::critical(|| {
                let level = record.level() as u8;
                let target = (
                    crate::vm::ka2pa(record.target().as_ptr()),
                    record.target().len(),
                );
                let file = record
                    .file()
                    .map(|x| (crate::vm::ka2pa(x.as_ptr()), x.len()));
                let line = record.line();
                let module_path = record
                    .module_path()
                    .map(|x| (crate::vm::ka2pa(x.as_ptr()), x.len()));
                let msg = format!("{}", record.args());
                let message = (crate::vm::ka2pa(msg.as_ptr()), msg.len());
                let record = common::LogRecord {
                    level,
                    target,
                    file,
                    line,
                    module_path,
                    message,
                };
                let p = crate::vm::ka2pa(&raw const record);
                unsafe {
                    crate::io::hypercall1(hypercall::LOG, p as u64);
                }
            });
        }
    }

    fn flush(&self) {
        todo!()
    }
}

pub static HOST: HostLogger = HostLogger;

pub fn symname(addr: *const ()) -> Option<(String, usize)> {
    unsafe {
        let mut buffer: Box<[u8]> = Box::new_uninit_slice(1024).assume_init();
        loop {
            let parts = (vm::ka2pa(buffer.as_mut_ptr()), buffer.len());
            let mut symtab = common::SymtabRecord {
                addr: addr as usize,
                file_buffer: parts,
                ..Default::default()
            };
            let p = crate::vm::ka2pa(&raw mut symtab);
            crate::io::hypercall1(hypercall::SYMNAME, p as u64);
            if !symtab.found {
                return None;
            }
            if symtab.file_len > buffer.len() {
                buffer = Box::new_uninit_slice(buffer.len() * 2).assume_init();
                continue;
            }
            let bytes = &buffer[..symtab.file_len];
            let name = core::str::from_utf8(bytes).expect("got back invalid UTF-8 from host");
            return Some((name.into(), addr as usize - symtab.addr));
        }
    }
}

pub fn memset(region: *mut [u8], value: u8) {
    let (p, n) = region.to_raw_parts();
    let p = vm::ka2pa(p);
    unsafe {
        crate::io::hypercall3(hypercall::MEMSET, p as u64, value as u64, n as u64);
    }
}

pub fn memclr(region: *mut [u8]) {
    let (p, n) = region.to_raw_parts();
    let p = vm::ka2pa(p);
    unsafe {
        crate::io::hypercall2(hypercall::MEMCLR, p as u64, n as u64);
    }
}

pub mod os {
    use crate::prelude::*;
    use common::protocol::*;

    pub fn argv() -> Vec<String> {
        let mut binding = crate::pipe::HOST.lock();
        let host = binding.get_mut().unwrap();
        let control::Response::Args(args) = host.request(&control::Request::GetArgs) else {
            panic!("bad response");
        };
        args
    }
}

use super::pipe::HostPipe;
unsafe fn get_pipe(data: common::protocol::control::PipeData) -> HostPipe {
    use common::pipe::{Pipe, Reader, Writer};
    let rxp: *const u8 = BuddyAllocator.from_offset(data.rx_ptr);
    let txp: *const u8 = BuddyAllocator.from_offset(data.tx_ptr);
    let rx = Arc::from_raw_in(core::ptr::from_raw_parts(rxp, data.rx_len), BuddyAllocator);
    let rx = Reader::from_inner(rx);
    let tx = Arc::from_raw_in(core::ptr::from_raw_parts(txp, data.tx_len), BuddyAllocator);
    let tx = Writer::from_inner(tx);
    let pipe = Pipe::from_inner(rx, tx);
    HostPipe::new(pipe)
}

pub mod net {
    use super::get_pipe;
    use crate::pipe::*;
    use crate::prelude::*;
    use common::protocol::*;

    pub struct TcpListener {
        pipe: KMutex<ListenerPipe>,
    }

    pub struct TcpStream {
        pipe: StreamPipe,
    }

    impl TcpListener {
        pub fn bind(ip: &[u8; 4], port: u16) -> TcpListener {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let ip = *ip;
            let control::Response::Pipe(id) = host.request(&control::Request::Listen { ip, port })
            else {
                todo!();
            };
            unsafe {
                TcpListener {
                    pipe: KMutex::new(ListenerPipe::new(get_pipe(id))),
                }
            }
        }

        pub fn accept(&self) -> TcpStream {
            let mut pipe = self.pipe.lock();
            let listener::Response::Pipe(id) = pipe.request(&listener::Request::Accept) else {
                todo!();
            };
            unsafe {
                TcpStream {
                    pipe: StreamPipe::new(get_pipe(id)),
                }
            }
        }
    }

    impl Drop for TcpListener {
        fn drop(&mut self) {
            let mut pipe = self.pipe.lock();
            let listener::Response::Ack = pipe.request(&listener::Request::Close) else {
                todo!();
            };
        }
    }

    impl TcpStream {
        pub fn connect(hostname: &str, port: u16) -> TcpStream {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let control::Response::Pipe(id) = host.request(&control::Request::Connect {
                host: hostname.into(),
                port,
            }) else {
                todo!();
            };
            unsafe {
                TcpStream {
                    pipe: StreamPipe::new(get_pipe(id)),
                }
            }
        }

        pub fn send(&mut self, bytes: &[u8]) -> usize {
            let stream::Response::Length(len) =
                self.pipe.request(&stream::Request::Send(bytes.into()))
            else {
                panic!("bad response");
            };
            len
        }

        pub fn recv(&mut self, bytes: &mut [u8]) -> usize {
            let stream::Response::Bytes(buf) =
                self.pipe.request(&stream::Request::Receive(bytes.len()))
            else {
                panic!("bad response");
            };
            bytes[..buf.len()].copy_from_slice(&buf);
            bytes.len()
        }

        pub fn close(self) {
            let _ = self;
        }
    }

    impl Drop for TcpStream {
        fn drop(&mut self) {
            let stream::Response::Ack = self.pipe.request(&stream::Request::Close) else {
                panic!("bad response");
            };
        }
    }

    impl !Sync for TcpStream {}

    impl core::fmt::Write for TcpStream {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let mut s = s.as_bytes();
            while !s.is_empty() {
                let len = self.send(s);
                s = &s[len..];
            }
            Ok(())
        }
    }
}

pub mod fs {
    use super::get_pipe;
    use crate::pipe::*;
    pub use common::protocol::file::Whence;
    use common::{protocol::control::FileMode, protocol::*};

    pub struct File {
        pipe: FilePipe,
    }

    /// Recursively creates a directory and all of its parents on the host, like
    /// `mkdir -p`.  Returns `true` on success.  Mirrors [`File::open`], but the
    /// host has nothing to stream back so it replies with a bare ack rather than
    /// a pipe.
    pub fn mkdir(path: &str) -> bool {
        let mut binding = crate::pipe::HOST.lock();
        let host = binding.get_mut().unwrap();
        matches!(
            host.request(&control::Request::Mkdir(path.into())),
            control::Response::Ack
        )
    }

    impl File {
        pub fn open(
            path: &str,
            read: bool,
            write: bool,
            create: bool,
            append: bool,
            truncate: bool,
        ) -> Option<File> {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let control::Response::Pipe(id) = host.request(&control::Request::Open(
                path.into(),
                FileMode {
                    read,
                    write,
                    create,
                    append,
                    truncate,
                },
            )) else {
                return None;
            };
            unsafe {
                Some(File {
                    pipe: FilePipe::new(get_pipe(id)),
                })
            }
        }

        pub fn close(self) {}

        pub fn read(&mut self, buf: &mut [u8]) -> usize {
            let file::Response::Bytes(bytes) = self.pipe.request(&file::Request::Read(buf.len()))
            else {
                panic!("bad response");
            };
            buf[..bytes.len()].copy_from_slice(&bytes);
            bytes.len()
        }

        pub fn read_exact(&mut self, mut buf: &mut [u8]) -> usize {
            let mut total = 0;
            while !buf.is_empty() {
                let n = self.read(buf);
                if n == 0 {
                    break;
                }
                buf = &mut buf[n..];
                total += n;
            }
            total
        }

        pub fn write(&mut self, buf: &[u8]) -> usize {
            let file::Response::Length(len) = self.pipe.request(&file::Request::Write(buf.into()))
            else {
                panic!("bad response");
            };
            len
        }

        pub fn write_exact(&mut self, mut buf: &[u8]) -> usize {
            let mut total = 0;
            while !buf.is_empty() {
                let n = self.write(buf);
                if n == 0 {
                    break;
                }
                buf = &buf[n..];
                total += n;
            }
            total
        }

        pub fn seek(&mut self, whence: Whence) -> u64 {
            let file::Response::Offset(offset) = self.pipe.request(&file::Request::Seek(whence))
            else {
                panic!("bad response");
            };
            offset
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            let file::Response::Ack = self.pipe.request(&file::Request::Close) else {
                panic!("bad response");
            };
        }
    }

    impl core::fmt::Write for File {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let mut s = s.as_bytes();
            while !s.is_empty() {
                let len = self.write(s);
                s = &s[len..];
            }
            Ok(())
        }
    }

    impl !Sync for File {}
}
