#![no_main]
#![no_std]

extern crate alloc;

extern crate kernel;

use core::arch::asm;

use kernel::{buddy::Page2MB, halt, shutdown, spinlock::SpinLock};

static DONE_COUNT: SpinLock<usize> = SpinLock::new(0);

#[repr(C)]
#[derive(Debug)]
struct RegisterFile {
    registers: [u64; 16],
    rip: u64,
    flags: u64,
    mode: u64,
}

impl RegisterFile {
    pub fn function(f: extern "C" fn() -> !, stack: &mut [u8]) -> RegisterFile {
        let mut regs = RegisterFile {
            registers: [0; 16],
            rip: f as usize as u64,
            flags: 0x202,
            mode: 1,
        };
        regs.registers[4] = unsafe { stack.as_ptr().add(stack.len()) as u64 };
        regs
    }

    pub fn step(&mut self) -> ExitStatus {
        let syscall_safe =
            self.registers[1] == self.rip && self.registers[11] == self.flags && self.mode == 1;
        if syscall_safe {
            unsafe { syscall_call_user(self) }
        } else {
            unsafe { isr_call_user(self) }
        }
    }
}

#[repr(C)]
#[derive(Debug)]
struct ExitStatus {
    code: u64,
    error: u64,
}

extern "C" {
    fn syscall_call_user(registers: &mut RegisterFile) -> ExitStatus;
    fn isr_call_user(registers: &mut RegisterFile) -> ExitStatus;
}

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    if cfg!(test) {
        shutdown();
    }

    let mut count = DONE_COUNT.lock();
    log::info!(
        "Hello from CPU {}/{} (n={}) (t={:?})!",
        kernel::cpuinfo::id(),
        kernel::cpuinfo::ncores(),
        *count,
        kernel::kvmclock::time_since_boot(),
    );
    *count += 1;
    if *count == kernel::cpuinfo::ncores() {
        log::info!("On core {}", kernel::cpuinfo::id());
        log::info!("All {} cores done!", kernel::cpuinfo::ncores());
        log::info!("Boot took {:?}", kernel::kvmclock::time_since_boot());

        log::info!("About to switch to user mode.");
        let mut stack = Page2MB::new();
        let mut regs = RegisterFile::function(umain, &mut *stack);
        loop {
            let result = regs.step();
            if result.code == 0x100 && regs.registers[7] == 0 {
                break;
            }
        }
        log::info!("Shutting down.");
        shutdown();
    }
    count.unlock();
    halt();
}

#[no_mangle]
extern "C" fn umain() -> ! {
    log::info!("In user mode!");
    let iters = 0x1000;
    let time = kernel::tsc::time(|| unsafe {
        for _ in 0..iters {
            asm!("int 0x80");
        }
    });
    log::info!("Software Interrupt took {:?}", time / iters);
    let time = kernel::tsc::time(|| unsafe {
        for _ in 0..iters {
            asm!("syscall", out("rcx")_, out("r11")_, in("rdi")1);
        }
    });
    log::info!("Syscall took {:?}", time / iters);
    unsafe { asm!("syscall", in("rdi") 0) };
    halt();
}
