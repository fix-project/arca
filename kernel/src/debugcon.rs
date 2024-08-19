use core::{fmt::Write, marker::PhantomData};

use crate::{io::outb, spinlock::SpinLock};

pub struct DebugConsole(PhantomData<()>);

impl !Sync for DebugConsole {}

impl Write for DebugConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            unsafe {
                outb(0xe9, c);
            }
        }
        Ok(())
    }
}

pub static CONSOLE: SpinLock<DebugConsole> = SpinLock::new(DebugConsole(PhantomData));

pub struct DebugLogger;

impl log::Log for DebugLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut con = CONSOLE.lock();

        if self.enabled(record.metadata()) {
            let _ = writeln!(
                con,
                "[{} {}:{}] {}",
                record.level(),
                record.file().unwrap_or("<unknown>"),
                record.line().unwrap_or(0),
                record.args(),
            );
        }
    }

    fn flush(&self) {
        todo!()
    }
}
