#![no_main]
#![no_std]

extern crate alloc;

extern crate kernel;

use core::arch::asm;

use kernel::{
    arca::Arca,
    buddy::Page2MB,
    cpu::{Register, CPU},
    halt,
    paging::{PageTable, PageTable1GB, PageTable512GB, PageTableEntry, Permissions},
    shutdown,
    spinlock::SpinLock,
};

static DONE_COUNT: SpinLock<usize> = SpinLock::new(0);

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

        let mut arca0 = Arca::new();
        arca0.registers_mut()[Register::RDI] = 0;
        arca0.registers_mut()[Register::RSP] = 1 << 21;
        arca0.registers_mut()[Register::RIP] = umain as usize as u64;
        let stack0 = Page2MB::new();
        let mut pd = PageTable1GB::new();
        pd[0].map(stack0.into(), Permissions::All);
        let mut pdpt = PageTable512GB::new();
        pdpt[0].chain(pd.into(), Permissions::All);
        arca0.mappings_mut().chain(pdpt.into(), Permissions::All);

        let mut arca1 = Arca::new();
        arca1.registers_mut()[Register::RDI] = 1;
        arca1.registers_mut()[Register::RSP] = 1 << 21;
        arca1.registers_mut()[Register::RIP] = umain as usize as u64;
        let stack1 = Page2MB::new();
        let mut pd = PageTable1GB::new();
        pd[0].map(stack1.into(), Permissions::All);
        let mut pdpt = PageTable512GB::new();
        pdpt[0].chain(pd.into(), Permissions::All);
        arca1.mappings_mut().chain(pdpt.into(), Permissions::All);

        let mut cpu = CPU.borrow_mut();
        let mut arca = arca0.load(&mut cpu);
        let mut other = arca1;
        log::info!("About to switch to user mode!");
        'runner: loop {
            let result = arca.run();
            if result.code == 0x100 || result.code == 0x80 {
                match arca.registers()[Register::RDI] {
                    0 => break 'runner,
                    1 => {
                        arca.swap(&mut other);
                    }
                    2 => {}
                    x => {
                        unimplemented!("syscall {x}");
                    }
                }
            } else {
                let registers = arca.registers();
                log::error!("unexpected exit: {result:?} with status {registers:#x?}");
                let mut cr2: u64;
                unsafe {
                    asm!("mov {cr2}, cr2", cr2=out(reg)cr2);
                }
                log::error!("CR2: {cr2:x}");
                shutdown();
            }
        }
        let _ = arca.unload();
        log::info!("Shutting down.");
        shutdown();
    }
    count.unlock();
    halt();
}

#[no_mangle]
unsafe extern "C" fn umain(id: u64) -> ! {
    log::info!("{id}: In user mode!");
    let iters = 0x1000;
    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            asm!("int 0x80", in("rdi")2);
        }
    });
    log::info!("{id}: Software Interrupt took {:?}", time / iters);
    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            asm!("syscall", out("rcx")_, out("r11")_, in("rdi")2);
        }
    });
    log::info!("{id}: Syscall took {:?}", time / iters);
    log::info!("{id}: Requesting switch.");
    asm!("syscall", out("rcx")_, out("r11")_, in("rdi")1);
    log::info!("{id}: Exiting.");
    asm!("syscall", in("rdi") 0);
    halt();
}
