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
            let record = Box::new(common::LogRecord {
                level,
                target,
                file,
                line,
                module_path,
                message,
            });
            let p = crate::vm::ka2pa(&raw const *record);
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
