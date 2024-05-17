use core::{
    ptr::addr_of_mut,
    sync::atomic::{AtomicBool, Ordering},
};

extern "C" {
    fn kmain(id: u32, bsp: bool, ncores: u32, multiboot: *const ()) -> !;
    static mut _sbss: u8;
    static mut _ebss: u8;
}

static WAIT_FOR_INIT: AtomicBool = AtomicBool::new(true);

#[no_mangle]
unsafe extern "C" fn rsstart(id: u32, bsp: bool, ncores: u32, multiboot: *const ()) {
    if bsp {
        let start = addr_of_mut!(_sbss);
        let end = addr_of_mut!(_ebss);
        let length = end.offset_from(start) as usize;
        let bss: &mut [u8] = core::slice::from_raw_parts_mut(start, length);
        bss.fill(0);
        WAIT_FOR_INIT.store(false, Ordering::Relaxed);
    } else {
        while WAIT_FOR_INIT.load(Ordering::Relaxed) {
            core::arch::x86_64::_mm_pause();
        }
    }
    kmain(id, bsp, ncores, multiboot);
}
