#![allow(unused)]

use common::{
    pipe::{DoorBell, PipeError},
    BuddyAllocator,
};

pub struct VMToHostDoorBell {
    addr: *mut u64,
    datamatch: u64,
}

impl VMToHostDoorBell {
    fn from_raw_parts(addr: u64, datamatch: u64) -> Self {
        let addr: *const u64 = BuddyAllocator.from_offset(addr as usize);
        let addr: *mut u64 = addr as *mut u64;
        Self { addr, datamatch }
    }
}

impl DoorBell for VMToHostDoorBell {
    fn ring(&self) {
        unsafe {
            core::ptr::write_volatile(self.addr, self.datamatch);
        }
    }
}
