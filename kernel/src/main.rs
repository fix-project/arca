#![no_main]
#![no_std]
#![feature(allocator_api)]

extern crate alloc;

extern crate kernel;

use core::arch::asm;

use kernel::{
    allocator::PHYSICAL_ALLOCATOR,
    arca::Arca,
    cpu::Register,
    page::{Page2MB, UniquePage},
    shutdown,
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
    log::info!("about to shut down");
    shutdown();
}

unsafe extern "C" fn umain() -> ! {
    log::info!("umain");
    let count = 0x1000;
    let time = kernel::kvmclock::time(|| {
        for _ in 0..count {
            asm!("syscall", in("rdi")1, out("rcx")_, out("r11")_);
        }
    });
    log::info!("syscall took: {} ns", time.as_nanos() / count);
    loop {
        asm!("syscall", in("rdi")0, out("rcx")_, out("r11")_);
    }
}
