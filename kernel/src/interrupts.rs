use core::cell::LazyCell;

use crate::buddy::UniquePage2MB;

#[core_local]
pub(crate) static INTERRUPT_STACK: LazyCell<UniquePage2MB> = LazyCell::new(UniquePage2MB::new);

#[repr(C)]
#[derive(Debug)]
struct RegisterFile {
    registers: [u64; 16],
    rip: u64,
    flags: u64,
    mode: u64,
}

const _: () = const {
    assert!(core::mem::size_of::<RegisterFile>() == 152);
};

#[repr(C)]
#[derive(Debug)]
struct IsrRegisterFile {
    registers: [u64; 16],
    isr: u64,
    code: u64,
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

extern "C" {
    fn isr_save_state_and_exit(isr: u64, error: u64, registers: &RegisterFile) -> !;
}

#[no_mangle]
unsafe extern "C" fn isr_entry(registers: &mut IsrRegisterFile) {
    if registers.cs & 0b11 == 0b11 {
        // return to user mode
        let regs = RegisterFile {
            registers: registers.registers,
            rip: registers.rip,
            flags: registers.rflags,
            mode: 1,
        };
        isr_save_state_and_exit(registers.isr, registers.code, &regs);
    }
    // supervisor mode
    if registers.isr == 0xd {
        log::error!("GP! faulting segment: {:x}", registers.code);
        crate::shutdown();
    }
    if registers.isr < 32 {
        panic!("unhandled exception: {:x?}", registers);
    }
    if registers.isr == 0x30 {
        crate::lapic::LAPIC.borrow_mut().clear_interrupt();
    } else {
        panic!("unhandled system ISR: {:x?}", registers);
    }
}
