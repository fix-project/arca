use super::*;

#[derive(Debug)]
pub struct UsedRing {
    next_read_index: u16,
    ring: *mut DeviceRing<UsedElement>,
}

unsafe impl Send for UsedRing {}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct UsedElement {
    id: u32,
    len: u32,
}

impl UsedRing {
    pub unsafe fn new(p: *mut DeviceRing<UsedElement>) -> Self {
        Self {
            next_read_index: 0,
            ring: p,
        }
    }

    pub fn pop(&mut self) -> Option<UsedElement> {
        let ring = unsafe { &mut *self.ring };
        let next_write = ring.next_write_index();
        if next_write == self.next_read_index {
            None
        } else {
            let result = ring.get(self.next_read_index);
            self.next_read_index = self.next_read_index.wrapping_add(1);
            Some(result)
        }
    }
}

impl UsedElement {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn len(&self) -> u32 {
        self.len
    }
}
