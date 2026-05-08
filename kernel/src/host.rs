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

pub mod net {
    use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use common::{
        hypercall::{self, TcpInfo},
        BuddyAllocator,
    };

    use crate::kthread;

    pub struct TcpListener {
        id: u64,
    }

    pub struct TcpStream {
        id: u64,
    }

    impl TcpListener {
        pub fn bind(ip: &[u8; 4], port: u16) -> TcpListener {
            let info = TcpInfo {
                ip: u32::from_ne_bytes(*ip),
                port,
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(
                    hypercall::TCP_LISTEN,
                    BuddyAllocator.to_offset(&info) as u64,
                )
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            TcpListener {
                id: info.id.load(Ordering::SeqCst),
            }
        }

        pub fn accept(&mut self) -> TcpStream {
            let info = TcpInfo {
                id: AtomicU64::new(self.id),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(
                    hypercall::TCP_ACCEPT,
                    BuddyAllocator.to_offset(&info) as u64,
                )
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            TcpStream {
                id: info.id.load(Ordering::SeqCst),
            }
        }
    }

    impl TcpStream {
        pub fn connect(ip: &[u8; 4], port: u16) -> TcpStream {
            let info = TcpInfo {
                ip: u32::from_ne_bytes(*ip),
                port,
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(
                    hypercall::TCP_CONNECT,
                    BuddyAllocator.to_offset(&info) as u64,
                )
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            TcpStream {
                id: info.id.load(Ordering::SeqCst),
            }
        }

        pub fn send(&mut self, bytes: &[u8]) -> usize {
            let info = TcpInfo {
                id: AtomicU64::new(self.id),
                buf: BuddyAllocator.to_offset(bytes.as_ptr()),
                len: AtomicUsize::new(bytes.len()),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(hypercall::TCP_SEND, BuddyAllocator.to_offset(&info) as u64)
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            info.len.load(Ordering::SeqCst)
        }

        pub fn recv(&mut self, bytes: &mut [u8]) -> usize {
            let info = TcpInfo {
                id: AtomicU64::new(self.id),
                buf: BuddyAllocator.to_offset(bytes.as_ptr()),
                len: AtomicUsize::new(bytes.len()),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(hypercall::TCP_RECV, BuddyAllocator.to_offset(&info) as u64)
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            info.len.load(Ordering::SeqCst)
        }

        pub fn close(self) {
            let _ = self;
        }
    }

    impl Drop for TcpStream {
        fn drop(&mut self) {
            let info = TcpInfo {
                id: AtomicU64::new(self.id),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(hypercall::TCP_CLOSE, BuddyAllocator.to_offset(&info) as u64)
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
        }
    }

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
    use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

    use common::{
        hypercall::{self, FileInfo},
        BuddyAllocator,
    };

    use crate::kthread;

    pub struct File {
        id: u64,
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
            let info = FileInfo {
                buf: BuddyAllocator.to_offset(path.as_ptr()),
                len: AtomicUsize::new(path.len()),
                read,
                write,
                create,
                append,
                truncate,
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(hypercall::FILE_OPEN, BuddyAllocator.to_offset(&info) as u64)
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            let id = info.id.load(Ordering::SeqCst);
            if id != 0 {
                Some(File { id })
            } else {
                None
            }
        }

        pub fn close(self) {}

        pub fn read(&mut self, buf: &mut [u8]) -> usize {
            let info = FileInfo {
                id: AtomicU64::new(self.id),
                buf: BuddyAllocator.to_offset(buf.as_ptr()),
                len: AtomicUsize::new(buf.len()),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(hypercall::FILE_READ, BuddyAllocator.to_offset(&info) as u64)
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            info.len.load(Ordering::SeqCst)
        }

        pub fn write(&mut self, buf: &[u8]) -> usize {
            let info = FileInfo {
                id: AtomicU64::new(self.id),
                buf: BuddyAllocator.to_offset(buf.as_ptr()),
                len: AtomicUsize::new(buf.len()),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(
                    hypercall::FILE_WRITE,
                    BuddyAllocator.to_offset(&info) as u64,
                )
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            info.len.load(Ordering::SeqCst)
        }

        pub fn seek(&mut self, offset: isize, whence: i64) -> usize {
            let info = FileInfo {
                id: AtomicU64::new(self.id),
                offset,
                whence,
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(hypercall::FILE_SEEK, BuddyAllocator.to_offset(&info) as u64)
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
            info.len.load(Ordering::SeqCst)
        }
    }

    impl Drop for File {
        fn drop(&mut self) {
            let info = FileInfo {
                id: AtomicU64::new(self.id),
                ..Default::default()
            };
            unsafe {
                crate::io::hypercall1(
                    hypercall::FILE_CLOSE,
                    BuddyAllocator.to_offset(&info) as u64,
                )
            };
            while !info.done.load(Ordering::SeqCst) {
                kthread::wfi();
            }
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
}
