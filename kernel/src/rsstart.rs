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
        let mut p: *mut u8 = addr_of_mut!(_sbss);
        while p < addr_of_mut!(_ebss) {
            p.write(0);
            p = p.add(1);
        }
        rsstart_gate = true;
    }
    kmain(id, bsp, ncores, multiboot);
}
