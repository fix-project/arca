#![no_main]
#![no_std]

extern crate alloc;

extern crate kernel;

use core::arch::asm;

use alloc::vec;
use kernel::{
    arca::{AddressSpace, Arca, Blob, MapConfig},
    buddy::{UniquePage2MB, UniquePage4KB},
    cpu::{Register, CPU},
    halt,
    paging::{PageTable, PageTable1GB, PageTable512GB, PageTableEntry},
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
    *count += 1;
    if *count == kernel::cpuinfo::ncores() {
        log::info!("On core {}", kernel::cpuinfo::id());
        log::info!("All {} cores done!", kernel::cpuinfo::ncores());
        log::info!("Boot took {:?}", kernel::kvmclock::time_since_boot());

        let iters = 0x100;
        let time = kernel::tsc::time(|| {
            for _ in 0..iters {
                let _ = Arca::new();
            }
        });
        log::info!("Creation took {:?}", time / iters);

        let mut data = unsafe { UniquePage4KB::zeroed().assume_init() };
        data[0..13].copy_from_slice(b"hello, world!");

        let mut pages = vec![data.into()];
        pages.extend(pages.clone());
        pages.extend(pages.clone());
        pages.extend(pages.clone());
        pages.extend(pages.clone());
        let blob = Blob { pages };
        log::info!("blob is {} bytes", blob.len());

        let mut arca0 = Arca::new();
        arca0.registers_mut()[Register::RDI] = 0;
        arca0.registers_mut()[Register::RSP] = 1 << 21;
        arca0.registers_mut()[Register::RIP] = umain as usize as u64;
        let stack0 = UniquePage2MB::new();
        let mut pd = PageTable1GB::new();
        pd[0].map_unique(stack0);
        let mut pdpt = PageTable512GB::new();
        pdpt[0].chain(pd.into());
        arca0.mappings_mut().chain(pdpt.into());

        run(arca0, &blob);
        log::info!("Shutting down.");
        shutdown();
    }
    count.unlock();
    halt();
}

fn run(target: Arca, blob: &Blob) {
    let mut cpu = CPU.borrow_mut();
    let mut arca = target.load(&mut cpu);
    loop {
        let result = arca.run();
        if result.code == 0x100 || result.code == 0x80 {
            match arca.registers()[Register::RDI] {
                0 => {
                    return;
                }
                1 => {}
                2 => {
                    let saved = arca.unload();
                    arca = saved.load(&mut cpu);
                }
                3 => {
                    let mut unloaded = arca.unload();
                    let mut forked = unloaded.clone();
                    forked.registers_mut()[Register::RAX] = 1;
                    unloaded.registers_mut()[Register::RAX] = 0;
                    core::mem::drop(cpu);
                    run(forked, blob);
                    cpu = CPU.borrow_mut();
                    arca = unloaded.load(&mut cpu);
                }
                4 => {
                    let addr = arca.registers()[Register::RSI];
                    arca.map_blob(
                        blob,
                        MapConfig {
                            offset: 0,
                            addr: addr as usize,
                            len: blob.len(),
                            unique: false,
                        },
                    );
                }
                5 => {
                    let addr = arca.registers()[Register::RSI];
                    let len = arca.registers()[Register::RDX] as usize;
                    arca.unmap(addr as usize, len);
                }
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
}

#[no_mangle]
unsafe extern "C" fn umain(id: u64) -> ! {
    log::info!("{id}: In user mode!");
    let iters = 0x1000;
    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            asm!("int 0x80", in("rdi")1);
        }
    });
    log::info!("{id}: Software Interrupt took {:?}", time / iters);

    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            asm!("syscall", out("rcx")_, out("r11")_, in("rdi")1);
        }
    });
    log::info!("{id}: Syscall took {:?}", time / iters);

    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            asm!("syscall", out("rcx")_, out("r11")_, in("rdi")2);
        }
    });
    log::info!("{id}: Reload took {:?}", time / iters);

    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            let mut x: u64;
            asm!("syscall", out("rcx")_, out("r11")_, in("rdi")3, out("rax")x);
            if x == 1 {
                asm!("syscall", in("rdi") 0);
            }
        }
    });
    log::info!("{id}: Forking took {:?}", time / iters);

    let time = kernel::tsc::time(|| {
        for _ in 0..iters {
            asm!("syscall", out("rcx")_, out("r11")_, in("rdi")4, in("rsi")0x400000);
        }
    });
    log::info!("{id}: Mapping took {:?}", time / iters);
    asm!("syscall", out("rcx")_, out("r11")_, in("rdi")5, in("rsi")0x401000, in("rdx")4096);
    let s = core::ffi::CStr::from_ptr(0x400000 as *const i8);
    log::info!("{id}: Blob Contents: {:?}", s);

    log::info!("{id}: Exiting.");
    asm!("syscall", in("rdi") 0);
    halt();
}
