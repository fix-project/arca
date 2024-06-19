#[thread_local]
#[no_mangle]
static mut SAVED_SS: u64 = 0;

#[thread_local]
#[no_mangle]
static mut SAVED_RSP: u64 = 0;

#[thread_local]
#[no_mangle]
pub static mut INTERRUPT_STACK: u64 = 0;

#[repr(C)]
#[derive(Debug)]
struct RegisterFile {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    fs_base: u64,
    gs_base: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[no_mangle]
unsafe extern "C" fn isr_handler(isr: u64, code: u64, _: &mut RegisterFile) {
    if isr < 32 {
        log::error!("got ISR {isr} with code {code}");
        crate::shutdown();
    }
}
