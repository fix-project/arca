use core::{
    arch::asm,
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::io::outb;

pub struct DebugConsole;

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

static CONSOLE_LOCK: AtomicBool = AtomicBool::new(false);

pub struct DebugLogger;

impl log::Log for DebugLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let mut con = DebugConsole;

        while CONSOLE_LOCK
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            unsafe {
                asm!("pause");
            }
        }

        if self.enabled(record.metadata()) {
            let _ = writeln!(
                con,
                "[{} {}:{}] {}",
                record.level(),
                record.file().unwrap_or("<unknown>"),
                record.line().unwrap_or(0),
                record.args()
            );
        }

        CONSOLE_LOCK.store(false, Ordering::SeqCst);
    }

    fn flush(&self) {
        todo!()
    }
}
