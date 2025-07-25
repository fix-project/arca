use core::fmt::Write as _;
use defs::*;

struct ExceptionWriter;

impl ExceptionWriter {
    pub fn reset(&self) {
        unsafe {
            arca_exception_reset();
        }
    }

    pub fn exit(&self) {
        unsafe {
            arca_exception_return();
        }
    }
}

impl core::fmt::Write for ExceptionWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let result = unsafe { arca_exception_append(s.as_ptr(), s.len()) };
        if result == 0 {
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

fn log(s: &str) {
    unsafe {
        arca_debug_log(s.as_ptr(), s.len());
    }
}

fn log_int(s: &str, x: u64) {
    unsafe {
        arca_debug_log_int(s.as_ptr(), s.len(), x);
    }
}

fn show(s: &str, x: i64) {
    unsafe {
        arca_debug_show(s.as_ptr(), s.len(), x);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    ExceptionWriter.reset();
    let _ = writeln!(ExceptionWriter, "{info}");
    ExceptionWriter.exit();
    loop {
        unsafe {
            core::arch::asm!("ud2");
        }
        core::hint::spin_loop();
    }
}
