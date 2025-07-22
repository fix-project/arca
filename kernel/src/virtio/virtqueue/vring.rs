#[repr(C)]
pub struct Ring<T: Copy> {
    flags: u16,
    idx: u16,
    ring: [T],
}

impl<T: Copy> Ring<T> {
    pub fn index(&self) -> u16 {
        unsafe { (&raw const self.idx).read_volatile() }
    }

    pub fn good_index(&self) -> usize {
        self.index() as usize % self.ring.len()
    }
}

#[repr(transparent)]
pub struct DeviceRing<T: Copy>(Ring<T>);

impl<T: Copy> DeviceRing<T> {
    pub fn next_write_index(&self) -> u16 {
        self.0.index()
    }

    pub fn get(&self, index: u16) -> T {
        let index = index as usize % self.0.ring.len();
        unsafe { (&raw const self.0.ring[index]).read_volatile() }
    }
}

#[repr(transparent)]
pub struct DriverRing<T: Copy>(Ring<T>);

impl<T: Copy> DriverRing<T> {
    pub unsafe fn push(&mut self, value: T) {
        (&raw mut self.0.ring[self.0.good_index()]).write_volatile(value);
        let new_index = self.0.index().wrapping_add(1);
        (&raw mut self.0.idx).write_volatile(new_index);
    }
}
