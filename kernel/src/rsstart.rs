use core::{
    ptr::addr_of_mut,
    sync::atomic::{AtomicBool, Ordering},
};

use log::LevelFilter;

use crate::{buddy, debugcon::DebugLogger, multiboot::MultibootInfo};

extern "C" {
    fn kmain(id: u32, bsp: bool, ncores: u32) -> !;
    static mut _sbss: u8;
    static mut _ebss: u8;
}

static WAIT_FOR_INIT: AtomicBool = AtomicBool::new(true);
static LOGGER: DebugLogger = DebugLogger;

#[no_mangle]
unsafe extern "C" fn _rsstart(id: u32, bsp: bool, ncores: u32, multiboot: *const MultibootInfo) {
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

        WAIT_FOR_INIT.store(false, Ordering::Relaxed);
    } else {
        while WAIT_FOR_INIT.load(Ordering::Relaxed) {
            core::arch::x86_64::_mm_pause();
        }
    }

    kmain(id, bsp, ncores);
}
