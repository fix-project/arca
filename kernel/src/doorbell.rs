#![allow(unused)]

use common::{
    message::traits::FixedMsg,
    pipe::{DoorBell, PipeError},
    BuddyAllocator,
};

pub struct VMToHostDoorBellInfo {
    inner: (u64, u64),
}

impl FixedMsg for VMToHostDoorBellInfo {
    const SIZE: usize = u64::SIZE + u64::SIZE;

    fn encode(&self, out: &mut [u8]) -> Result<(), common::message::traits::Error> {
        if out.len() != Self::SIZE {
            Err(common::message::traits::Error::SerializerError)
        } else {
            let (a, b) = out.split_at_mut(u64::SIZE);
            self.inner.0.encode(a)?;
            self.inner.1.encode(b)
        }
    }

    fn decode(input: &[u8]) -> Result<Self, common::message::traits::Error> {
        if input.len() != Self::SIZE {
            Err(common::message::traits::Error::ParserError)
        } else {
            let (a, b) = input.split_at(u64::SIZE);
            Ok(Self {
                inner: (u64::decode(a)?, u64::decode(b)?),
            })
        }
    }
}

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

    pub fn from_door_bell_info(info: VMToHostDoorBellInfo) -> Self {
        Self::from_raw_parts(info.inner.0, info.inner.1)
    }
}

impl DoorBell for VMToHostDoorBell {
    fn ring(&self) {
        unsafe {
            core::ptr::write_volatile(self.addr, self.datamatch);
        }
    }
}
