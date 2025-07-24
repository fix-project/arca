#[repr(C)]
pub struct Ring<T: Copy> {
    flags: u16,
    idx: u16,
    ring: [T],
}

impl<T: Copy> Ring<T> {
    pub fn raw_index(&self) -> u16 {
        unsafe { (&raw const self.idx).read_volatile() }
    }

    pub fn len(&self) -> usize {
        return self.ring.len();
    }

    pub fn get(&self, idx: u16) -> T {
        unsafe { (&raw const self.ring[idx as usize % self.len()]).read_volatile() }
    }

    pub fn set(&mut self, idx: u16, value: T) {
        unsafe {
            (&raw mut self.ring[idx as usize % self.len()]).write_volatile(value);
        }
    }

    pub fn inc(&mut self) {
        unsafe {
            (&raw mut self.idx).write_volatile(self.raw_index().wrapping_add(1));
        }
    }
}

#[derive(Debug)]
pub struct DeviceRing<T: Copy> {
    name: &'static str,
    next_read: u16,
    ring: *mut Ring<T>,
}

impl<T: Copy> DeviceRing<T> {
    pub unsafe fn new(name: &'static str, ring: *mut Ring<T>) -> Self {
        Self {
            name,
            next_read: 0,
            ring,
        }
    }

    fn ring(&self) -> &Ring<T> {
        unsafe { &*self.ring }
    }

    pub unsafe fn recv(&mut self) -> Option<T> {
        if self.next_read != self.ring().raw_index() {
            log::debug!("{} is receiving", self.name);
            let value = self.ring().get(self.next_read);
            self.next_read = self.next_read.wrapping_add(1);
            Some(value)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct DriverRing<T: Copy> {
    name: &'static str,
    ring: *mut Ring<T>,
}

impl<T: Copy> DriverRing<T> {
    pub unsafe fn new(name: &'static str, ring: *mut Ring<T>) -> Self {
        Self { name, ring }
    }

    fn ring(&self) -> &Ring<T> {
        unsafe { &*self.ring }
    }

    fn ring_mut(&mut self) -> &mut Ring<T> {
        unsafe { &mut *self.ring }
    }

    pub unsafe fn send(&mut self, value: T) {
        log::debug!("{} is sending", self.name);
        let idx = self.ring().raw_index();
        self.ring_mut().set(idx, value);
        self.ring_mut().inc();
        crate::io::outl(0xf4, 0);
    }
}

unsafe impl<T: Copy> Send for DeviceRing<T> {}
unsafe impl<T: Copy> Send for DriverRing<T> {}
