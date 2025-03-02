// use core::cell::LazyCell;

use core::{
    cell::LazyCell,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::prelude::*;

#[core_local]
pub(crate) static INTERRUPT_STACK: LazyCell<*mut Page2MB> = LazyCell::new(|| {
    let stack = Box::<Page2MB>::new_uninit();
    unsafe {
        let stack = stack.assume_init();
        Box::leak(stack) as *mut Page2MB
    }
});

#[core_local]
#[no_mangle]
pub(crate) static SEGFAULT_ESCAPE_ADDR: AtomicPtr<fn(u64, u64) -> !> =
    AtomicPtr::new(core::ptr::null_mut());

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
        if registers.isr == 0x20 {
            crate::lapic::LAPIC.borrow_mut().clear_interrupt();
        }
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
        if registers.code == 0 {
            panic!("Supervisor GP @ {:p}!", registers.rip as *mut (),);
        } else {
            panic!(
                "Supervisor GP @ {:p}! faulting segment: {:#x}",
                registers.rip as *mut (), registers.code,
            );
        }
    } else if registers.isr == 0xe {
        // page fault

        // it could be caused by copy_user_to_kernel or copy_kernel_to_user, which is okay
        let escape = SEGFAULT_ESCAPE_ADDR.load(Ordering::SeqCst);

        if !escape.is_null() {
            log::warn!(
                "user program provided invalid address to kernel: {:p}",
                crate::registers::read_cr2() as *const u8,
            );
            registers.rip = escape as usize as u64;
            return;
        }
        panic!(
            "unhandled page fault ({:b}) @ {:p} from RIP={:p}:",
            registers.code,
            crate::registers::read_cr2() as *const u8,
            registers.rip as *const u8,
        );
    }
    if registers.isr < 32 {
        panic!("unhandled exception: {:x?}", registers);
    }
    if registers.isr == 0x20 {
        // crate::allocator::PHYSICAL_ALLOCATOR.try_replenish();
        crate::lapic::LAPIC.borrow_mut().clear_interrupt();
        log::error!("kernel tick");
    } else {
        panic!("unhandled system ISR: {:x?}", registers);
    }
}
