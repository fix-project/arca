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

// The prebuilt System V alloc library references this symbol even with panic=abort.
// Arca has no unwinder; reaching a personality routine is a process fault.
#[unsafe(no_mangle)]
extern "C" fn rust_eh_personality() -> ! {
    loop {
        unsafe { core::arch::asm!("ud2") };
    }
}

#[unsafe(no_mangle)]
extern "C" fn _Unwind_Resume(_: *mut core::ffi::c_void) -> ! {
    rust_eh_personality()
}
