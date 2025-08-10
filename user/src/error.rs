use arcane::*;

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
    loop {
        unsafe {
            core::arch::asm!("ud2");
        }
        core::hint::spin_loop();
    }
}
