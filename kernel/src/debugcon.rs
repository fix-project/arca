use core::{fmt::Write, marker::PhantomData};

use crate::{io::inb, io::outb, spinlock::SpinLock};

pub struct DebugConsole(PhantomData<()>);

impl DebugConsole {
    pub fn write<T: AsRef<[u8]>>(&mut self, bytes: T) {
        for c in bytes.as_ref() {
            unsafe {
                outb(0xe9, *c);
            }
        }
    }

    pub fn read_byte(&mut self) -> u8 {
        unsafe { inb(0xe9) }
    }
}

impl !Sync for DebugConsole {}

impl Write for DebugConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s);
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
        crate::interrupts::critical(|| {
            let mut con = CONSOLE.lock();

            if self.enabled(record.metadata()) {
                let _ = writeln!(
                    con,
                    "[{:>5}({:02}) {}:{}] {}",
                    record.level(),
                    crate::coreid(),
                    record.file().unwrap_or("<unknown>"),
                    record.line().unwrap_or(0),
                    record.args(),
                );
            }
        });
    }

    fn flush(&self) {
        todo!()
    }
}

pub static DEBUG: DebugLogger = DebugLogger;
