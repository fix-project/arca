use super::*;

#[derive(Debug)]
pub struct AvailRing {
    ring: *mut DriverRing<DescriptorIndex>,
}

unsafe impl Send for AvailRing {}

impl AvailRing {
    pub unsafe fn new(p: *mut DriverRing<DescriptorIndex>) -> Self {
        Self { ring: p }
    }
    pub unsafe fn push(&mut self, idx: DescriptorIndex) {
        let ring = unsafe { &mut *self.ring };
        ring.push(idx);
        crate::io::outl(0xf4, 0);
    }
}
