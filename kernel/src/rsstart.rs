use core::ptr::addr_of_mut;

extern "C" {
    fn kmain(id: u8, bsp: bool, ncores: u8, multiboot: *const ()) -> !;
    static mut rsstart_gate: bool;
    static mut _sbss: u8;
    static mut _ebss: u8;
}

#[no_mangle]
unsafe extern "C" fn rsstart(id: u8, bsp: bool, ncores: u8, multiboot: *const ()) -> ! {
    if bsp {
        let start = addr_of_mut!(_sbss);
        let end = addr_of_mut!(_ebss);
        let length = end.offset_from(start) as usize;
        let bss: &mut [u8] = core::slice::from_raw_parts_mut(start, length);
        bss.fill(0);
        rsstart_gate = true;
    }
    kmain(id, bsp, ncores, multiboot);
}
