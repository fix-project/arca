use core::{arch::asm, cell::LazyCell};

use crate::buddy::Page2MB;

#[core_local]
pub(crate) static INTERRUPT_STACK: LazyCell<Page2MB> =
    LazyCell::new(|| Page2MB::new().expect("could not allocate interrupt stack"));

#[repr(C)]
#[derive(Debug)]
struct RegisterFile {
    registers: [u64; 16],
    rip: u64,
    flags: u64,
    mode: u64,
}

#[repr(C)]
#[derive(Debug)]
struct ExitStatus {
    code: u64,
    error: u64,
}

#[no_mangle]
unsafe extern "C" fn isr_entry(status: &mut ExitStatus, regs: &mut RegisterFile) {
    if status.code < 32 {
        log::info!("got ISR {status:x?} with {regs:x?}");
    }
    if status.code == 14 {
        let mut cr2: u64;
        asm!("mov {cr2}, cr2", cr2=out(reg)cr2);
        log::info!("faulting address: {cr2:#x}");
    }
    if regs.mode == 0 {
        log::error!("system-level ISR!");
        crate::shutdown();
    }
}
