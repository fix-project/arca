use core::cell::LazyCell;

use crate::buddy::Page2MB;

#[core_local]
#[no_mangle]
#[used]
pub(crate) static mut INTERRUPT_STACK_TOP: *mut u8 = core::ptr::null_mut();

#[core_local]
#[no_mangle]
#[used]
static mut SAVED_RSP: *mut u8 = core::ptr::null_mut();

#[core_local]
pub(crate) static INTERRUPT_STACK: LazyCell<Page2MB> = LazyCell::new(|| {
    let p = Page2MB::new().expect("could not allocate interrupt stack");
    let base = p.as_ptr() as usize;
    unsafe {
        *INTERRUPT_STACK_TOP = (base + p.len()) as *mut u8;
    }
    p
});

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct InterruptRegisterFile {
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
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct SyscallRegisterFile {
    rax: u64,
    rbx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rip: u64,
    rflags: u64,
    rsp: u64,
}

impl From<&SyscallRegisterFile> for InterruptRegisterFile {
    fn from(regs: &SyscallRegisterFile) -> Self {
        InterruptRegisterFile {
            rax: regs.rax,
            rbx: regs.rbx,
            rcx: regs.rip,
            rdx: regs.rdx,
            rsi: regs.rsi,
            rdi: regs.rdi,
            rbp: regs.rbp,
            r8: regs.r8,
            r9: regs.r9,
            r10: regs.r10,
            r11: regs.rflags,
            r12: regs.r12,
            r13: regs.r13,
            r14: regs.r14,
            r15: regs.r15,
            rip: regs.rip,
            rflags: regs.rflags,
            rsp: regs.rsp,
            ss: 0x23,
            cs: 0x2b,
        }
    }
}

#[allow(dead_code)]
extern "C" {
    pub fn syscall_exit(regs: &SyscallRegisterFile) -> !;
    pub fn isr_exit(regs: &InterruptRegisterFile) -> !;
}

#[no_mangle]
unsafe extern "C" fn isr_entry(isr: u64, code: u64, _: &mut InterruptRegisterFile) {
    if isr < 32 {
        log::error!("got ISR {isr} with code {code}");
        crate::shutdown();
    }
}

#[no_mangle]
unsafe extern "C" fn syscall_entry(_: &mut SyscallRegisterFile) {}
