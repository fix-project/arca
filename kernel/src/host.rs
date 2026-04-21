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
    use common::{hypercall, BuddyAllocator};

    pub struct TcpListener {
        id: usize,
    }

    pub struct TcpStream {
        id: usize,
    }

    impl TcpListener {
        pub fn bind(ip: &[u8; 4], port: u16) -> TcpListener {
            TcpListener {
                id: unsafe {
                    crate::io::hypercall2(
                        hypercall::TCP_LISTEN,
                        u32::from_ne_bytes(*ip) as u64,
                        port as u64,
                    ) as usize
                },
            }
        }

        pub fn accept(&self) -> TcpStream {
            TcpStream {
                id: unsafe {
                    crate::io::hypercall1(hypercall::TCP_ACCEPT, self.id as u64) as usize
                },
            }
        }
    }

    impl TcpStream {
        pub fn connect(ip: &[u8; 4], port: u16) -> TcpStream {
            TcpStream {
                id: unsafe {
                    crate::io::hypercall2(
                        hypercall::TCP_CONNECT,
                        u32::from_ne_bytes(*ip) as u64,
                        port as u64,
                    ) as usize
                },
            }
        }

        pub fn send(&self, bytes: &[u8]) -> usize {
            unsafe {
                crate::io::hypercall3(
                    hypercall::TCP_SEND,
                    self.id as u64,
                    BuddyAllocator.to_offset(bytes.as_ptr()) as usize as u64,
                    bytes.len() as u64,
                ) as usize
            }
        }

        pub fn recv(&self, bytes: &[u8]) -> usize {
            unsafe {
                crate::io::hypercall3(
                    hypercall::TCP_RECV,
                    self.id as u64,
                    BuddyAllocator.to_offset(bytes.as_ptr()) as usize as u64,
                    bytes.len() as u64,
                ) as usize
            }
        }

        pub fn close(self) {
            let _ = self;
        }
    }

    impl Drop for TcpStream {
        fn drop(&mut self) {
            unsafe {
                crate::io::hypercall1(hypercall::TCP_CLOSE, self.id as u64);
            }
        }
    }
}
