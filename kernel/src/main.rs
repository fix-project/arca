#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;

extern crate kernel;

use core::arch::asm;

use kernel::{
    allocator::PHYSICAL_ALLOCATOR,
    cpu::Register,
    page::{Page2MB, UniquePage},
    shutdown,
    types::Arca,
};

#[no_mangle]
#[inline(never)]
extern "C" fn kmain() -> ! {
    log::info!("kmain");

    let stack = UniquePage::<Page2MB>::new_uninit_in(&PHYSICAL_ALLOCATOR);
    let stack_end = UniquePage::into_raw(stack).wrapping_offset(1) as *mut u8;
    let mut arca = Arca::new();
    arca.registers_mut()[Register::RIP] = umain as usize as u64;
    arca.registers_mut()[Register::RSP] = stack_end as usize as u64;
    let mut cpu = kernel::cpu::CPU.borrow_mut();
    let mut arca = arca.load(&mut cpu);
    let result = loop {
        let result = arca.run();
        if result.code == 0x100 && arca.registers()[Register::RDI] == 0 {
            break result;
        }
    };
    log::info!("done: {:?}", result);
    shutdown();
}

#[inline(always)]
unsafe extern "C" fn syscall(num: u64, a0: u64, a1: u64) -> u64 {
    let mut val: u64;
    asm!("syscall", in("rdi")num, in("rsi")a0, in("rdx")a1, out("rax")val, out("rcx")_, out("r11")_);
    val
}

unsafe extern "C" fn umain() -> ! {
    log::info!("umain");
    let count = 0x1000;
    let time = kernel::kvmclock::time(|| {
        for _ in 0..count {
            syscall(1, 0, 0);
        }
    });
    log::info!("syscall took: {} ns", time.as_nanos() / count);
    loop {
        syscall(0, 0, 0);
    }
}
