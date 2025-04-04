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
pub(crate) struct IsrRegisterFile {
    pub registers: [u64; 16],
    pub isr: u64,
    pub code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

extern "C" {
    fn isr_save_state_and_exit(isr: u64, error: u64, registers: &RegisterFile) -> !;
}

#[no_mangle]
unsafe extern "C" fn isr_entry(registers: &mut IsrRegisterFile) {
    if registers.isr == 2 {
        log::error!("{} got an NMI!!!", crate::coreid());
        let mut i = 0;
        crate::profile::backtrace_from(registers.registers[5] as *const _, |rip| {
            if let Some((name, offset)) = crate::host::symname(rip as *const ()) {
                log::info!(
                    "CPU{} - {i}: {name}+{offset:#x} ({:p})!",
                    crate::coreid(),
                    rip
                );
            } else {
                log::info!("CPU{} - {i}: RIP={:p}!", crate::coreid(), rip);
            }
            i += 1;
        });
        if let Some((name, offset)) = crate::host::symname(registers.rip as *const ()) {
            panic!(
                "NMI on {} @ RIP={name}+{offset:#x} ({:p})!",
                crate::coreid(),
                registers.rip as *mut (),
            );
        } else {
            panic!(
                "NMI on {} @ RIP={:p}!",
                crate::coreid(),
                registers.rip as *mut (),
            );
        }
    }
    if registers.isr == 0x30 {
        // TLB Shootdown
        crate::tlb::handle_shootdown();
        return;
    }
    if registers.cs & 0b11 == 0b11 {
        if registers.isr == 0x20 {
            crate::profile::tick(registers);
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
            if let Some((name, offset)) = crate::host::symname(registers.rip as *const ()) {
                panic!(
                    "Supervisor GP @ {name}+{offset:#x} ({:p})!",
                    registers.rip as *mut (),
                );
            } else {
                panic!("Supervisor GP @ {:p}!", registers.rip as *mut (),);
            }
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
            log::debug!(
                "user program provided invalid address to kernel: {:p}",
                crate::registers::read_cr2() as *const u8,
            );
            registers.rip = escape as usize as u64;
            return;
        }
        if let Some((name, offset)) = crate::host::symname(registers.rip as *const ()) {
            panic!(
                "unhandled page fault ({:b}) @ {:p} from RIP={:p} ({}+{:#x}):",
                registers.code,
                crate::registers::read_cr2() as *const u8,
                registers.rip as *const u8,
                name,
                offset
            );
        } else {
            panic!(
                "unhandled page fault ({:b}) @ {:p} from RIP={:p}:",
                registers.code,
                crate::registers::read_cr2() as *const u8,
                registers.rip as *const u8,
            );
        }
    }
    if registers.isr < 32 {
        panic!("unhandled exception: {:x?}", registers);
    }
    if registers.isr == 0x20 {
        // crate::allocator::PHYSICAL_ALLOCATOR.try_replenish();
        crate::profile::tick(registers);
        crate::lapic::LAPIC.borrow_mut().clear_interrupt();
    } else {
        panic!("unhandled system ISR: {:x?}", registers);
    }
}
