use core::fmt::Write;

use arcane::*;

use crate::buffer::Buffer;

pub fn log(s: impl AsRef<[u8]>) {
    let s = s.as_ref();
    unsafe {
        arca_debug_log(s.as_ptr(), s.len());
    }
}

pub fn log_int(s: &str, x: u64) {
    unsafe {
        arca_debug_log_int(s.as_ptr(), s.len(), x);
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let mut buf: Buffer<1024> = Buffer::new();
    write!(&mut buf, "{info}");
    log(&*buf);
    loop {
        unsafe {
            core::arch::asm!("ud2");
        }
        core::hint::spin_loop();
    }
}
