use core::{
    cell::LazyCell,
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
    time::Duration,
};

use crate::{kvmclock, prelude::*};

pub(crate) static INTERRUPTED: AtomicBool = AtomicBool::new(false);

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
    fn get_if() -> bool;
}

pub fn enabled() -> bool {
    unsafe { get_if() }
}

pub unsafe fn disable() {
    core::arch::asm!("cli");
}

pub unsafe fn enable() {
    core::arch::asm!("sti");
}

pub fn critical<T>(f: impl FnOnce() -> T) -> T {
    let old = enabled();
    unsafe {
        disable();
    }
    let y = f();
    if old {
        unsafe {
            enable();
        }
    }
    y
}

pub fn must_be_enabled() {
    assert!(enabled());
}

pub fn must_be_disabled() {
    assert!(!enabled());
}

#[no_mangle]
unsafe extern "C" fn isr_entry(registers: &mut IsrRegisterFile) {
    must_be_disabled();
    if registers.isr == 0x30 {
        // log::warn!("got interrupt from virtio");
        INTERRUPTED.store(true, Ordering::Relaxed);
        crate::lapic::LAPIC.borrow_mut().clear_interrupt();
        return;
    }
    if registers.isr == 0x31 {
        INTERRUPTED.store(true, Ordering::Relaxed);
        if kvmclock::time_since_boot() > Duration::from_secs(1) {
            log::error!("got ^C interrupt");
            let mut console = crate::debugcon::CONSOLE.lock();
            let _ = writeln!(&mut *console, "----- BACKTRACE -----");
            let mut i = 0;
            crate::iprofile::backtrace(|addr, decoded| {
                if i > 0 {
                    if let Some((symname, offset)) = decoded {
                        let _ = writeln!(&mut *console, "{i}. {addr:#p} - {symname}+{offset:#x}");
                    } else {
                        let _ = writeln!(&mut *console, "{i}. {addr:#p}");
                    }
                }
                i += 1;
            });
            let _ = writeln!(&mut *console, "------ PROFILE ------");
            crate::iprofile::log(20);
            let _ = writeln!(&mut *console, "------ RUNTIME ------");
            crate::rt::profile();
            let _ = writeln!(&mut *console, "---------------------");
            crate::iprofile::reset();
            crate::rt::reset_stats();
        }
        // crate::shutdown();
        crate::lapic::LAPIC.borrow_mut().clear_interrupt();
        return;
    }
    if registers.cs & 0b11 == 0b11 {
        if registers.isr == 0x20 {
            INTERRUPTED.store(true, Ordering::Relaxed);
            crate::iprofile::tick(registers);
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
        INTERRUPTED.store(true, Ordering::Relaxed);
        crate::iprofile::tick(registers);
        crate::lapic::LAPIC.borrow_mut().clear_interrupt();
    } else {
        panic!("unhandled system ISR: {:x?}", registers);
    }
}
