#![allow(unused)]

use crate::vm;
use common::{pipe::DoorBell, protocol::control::VMToHostDoorBellData, BuddyAllocator};

#[derive(Debug)]
struct SendPtr(*mut u64);

/// #Safety
///
/// The ptr remains valid during the lifetime of SendPtr
unsafe impl Send for SendPtr {}

#[derive(Debug)]
pub struct VMToHostDoorBell {
    addr: SendPtr,
    datamatch: u64,
}

impl VMToHostDoorBell {
    /// #Safety
    ///
    /// raw must corresponds to a into_inner call on the vmm side on a VMToHostDoorBell
    pub unsafe fn from_raw_parts(raw: VMToHostDoorBellData) -> Self {
        let addr: *mut u64 = vm::pa2ka(raw.addr.try_into().unwrap());
        Self {
            addr: SendPtr(addr),
            datamatch: raw.datamatch,
        }
    }
}

impl DoorBell for VMToHostDoorBell {
    fn ring(&self) {
        unsafe {
            core::ptr::write_volatile(self.addr.0, self.datamatch);
        }
    }
}
