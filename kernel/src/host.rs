use crate::prelude::*;

use alloc::format;

pub struct HostLogger;

impl log::Log for HostLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
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
                crate::io::outl(1, p as u32);
                crate::io::outl(2, (p >> 32) as u32);
            }
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
            crate::io::outl(3, p as u32);
            crate::io::outl(4, (p >> 32) as u32);
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
