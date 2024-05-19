use core::{
    ptr::addr_of_mut,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use log::LevelFilter;

use crate::{
    buddy::{self, Page2MB},
    debugcon::DebugLogger,
    multiboot::MultibootInfo,
};

extern "C" {
    fn kmain() -> !;
    static mut _sbss: u8;
    static mut _ebss: u8;
}

static LOGGER: DebugLogger = DebugLogger;
static WAIT_FOR_INIT: AtomicU32 = AtomicU32::new(1);

#[no_mangle]
unsafe extern "C" fn _rsstart(
    id: u32,
    bsp: bool,
    ncores: u32,
    multiboot: *const MultibootInfo,
) -> *mut u8 {
    if bsp {
        let start = addr_of_mut!(_sbss);
        let end = addr_of_mut!(_ebss);
        let length = end.offset_from(start) as usize;
        let bss: &mut [u8] = core::slice::from_raw_parts_mut(start, length);
        bss.fill(0);

        let _ = log::set_logger(&LOGGER);
        log::set_max_level(LevelFilter::Info);

        let multiboot: &MultibootInfo = &*multiboot;
        log::info!("{:#x?}", multiboot.cmdline());
        let mmap = multiboot
            .memory_map()
            .expect("could not get memory map from bootloader");

        buddy::init(mmap);

        // WAIT_FOR_INIT.store(false, Ordering::Relaxed);
    } else {
        while WAIT_FOR_INIT.load(Ordering::SeqCst) <= id {
            core::arch::x86_64::_mm_pause();
        }
    }

    log::set_max_level(LevelFilter::Debug);
    let stack = Page2MB::new().expect("could not allocate stack");
    log::debug!("CPU {} is using {:p}+2MB as %rsp", id, stack.physical());

    let stack_bottom = stack.kernel();
    let stack_top = stack_bottom.add(0x200000);
    core::mem::forget(stack);
    WAIT_FOR_INIT.fetch_add(1, Ordering::SeqCst);

    stack_top
}

#[no_mangle]
unsafe extern "C" fn _rscontinue() -> ! {
    kmain();
}
