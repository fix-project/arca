use crate::handles::{self, Reference};

pub const BASE: usize = crate::rt::HANDLE_BASE;
const CAPACITY: u32 = (crate::rt::PAYLOAD_SIZE / fixhandle::HANDLE_SIZE) as u32;
static mut SLOTS: fixhandle::descriptors::Descriptors = fixhandle::descriptors::Descriptors::new();

pub fn initialize(value: Reference) -> u32 {
    assert_eq!(value as usize, BASE);
    unsafe {
        assert_eq!((&mut *(&raw mut SLOTS)).allocate(CAPACITY), Some(0));
    }
    0
}

pub fn address(index: u32) -> Reference {
    unsafe {
        assert!((&*(&raw const SLOTS)).contains(index), "closed descriptor");
    }
    (BASE + index as usize * fixhandle::HANDLE_SIZE) as Reference
}

pub fn allocate() -> u32 {
    unsafe {
        let length = (&*(&raw const SLOTS)).len();
        let index = (&mut *(&raw mut SLOTS))
            .allocate(CAPACITY)
            .expect("descriptor table exhausted");
        let cell = BASE + index as usize * fixhandle::HANDLE_SIZE;
        if index == length && cell.is_multiple_of(4096) {
            crate::rt::map_region(cell as *mut u8, 4096, arcane::__MODE_read_write)
                .expect("descriptor allocation failed");
        }
        index
    }
}

pub fn insert(value: Reference) -> u32 {
    let index = allocate();
    unsafe { handles::copy_handle(handles::address(value), address(index) as usize, 0) };
    index
}

pub fn close(index: u32) {
    let cell = address(index);
    unsafe {
        core::ptr::write_bytes(cell.cast::<u8>(), 0, fixhandle::HANDLE_SIZE);
        assert!((&mut *(&raw mut SLOTS)).close(index));
    }
}

pub fn range() -> (usize, usize) {
    (BASE, unsafe { (&*(&raw const SLOTS)).len() } as usize)
}
