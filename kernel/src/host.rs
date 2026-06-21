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
        let Response::Args(args) = host.request(&Request::GetArgs).unwrap() else {
            panic!("bad response");
        };
        args
    }
}

pub mod net {
    use common::{
        protocol::*,
    };

    pub struct TcpListener {
        id: ListenerDescriptor,
    }

    pub struct TcpStream {
        id: StreamDescriptor,
    }

    impl TcpListener {
        pub fn bind(ip: &[u8; 4], port: u16) -> TcpListener {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let ip = *ip;
            let Response::Listener(id) = host.request(&Request::Listen { ip, port }).unwrap() else {
                todo!();
            };
            TcpListener { id }
        }

        pub fn accept(&self) -> TcpStream {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Stream(id) = host.request(&Request::Accept(self.id.clone())).unwrap() else {
                todo!();
            };
            TcpStream { id }
        }
    }

    impl Drop for TcpListener {
        fn drop(&mut self) {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Ack = host.request(&Request::Accept(self.id.clone())).unwrap() else {
                todo!();
            };
        }
    }

    impl TcpStream {
        pub fn connect(hostname: &str, port: u16) -> TcpStream {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Stream(id) = host.request(&Request::Connect{host: hostname.into(), port}).unwrap() else {
                todo!();
            };
            TcpStream { id }
        }

        pub fn send(&mut self, bytes: &[u8]) -> usize {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Length(len) = host.request(&Request::Send(self.id.clone(), bytes.into())).unwrap() else {
                panic!("bad response");
            };
            len
        }

        pub fn recv(&mut self, bytes: &mut [u8]) -> usize {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Bytes(buf) = host.request(&Request::Receive(self.id.clone(), bytes.len())).unwrap() else {
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
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Ack = host.request(&Request::Disconnect(self.id.clone())).unwrap() else {
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
    use common::{
        protocol::*,
    };

    pub struct File {
        id: FileDescriptor,
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
            let Response::File(id) = host.request(&Request::Open(path.into(), FileMode {
                read,
                write,
                create,
                append,
                truncate,
            })).unwrap() else {
                return None;
            };
            Some(File { id })
        }

        pub fn close(self) { }

        pub fn read(&mut self, buf: &mut [u8]) -> usize {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Bytes(bytes) = host.request(&Request::Read(self.id.clone(), buf.len())).unwrap() else {
                panic!("bad response");
            };
            buf[..bytes.len()].copy_from_slice(&bytes);
            bytes.len()
        }

        pub fn write(&mut self, buf: &[u8]) -> usize {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Length(len) = host.request(&Request::Write(self.id.clone(), buf.into())).unwrap() else {
                panic!("bad response");
            };
            len
        }

        pub fn seek(&mut self, whence: Whence) -> u64 {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Offset(offset) = host.request(&Request::Seek(self.id.clone(), whence)).unwrap() else {
                panic!("bad response");
            };
            offset
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            let mut binding = crate::pipe::HOST.lock();
            let host = binding.get_mut().unwrap();
            let Response::Ack = host.request(&Request::Close(self.id.clone())).unwrap() else {
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
