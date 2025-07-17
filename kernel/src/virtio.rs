use core::cell::OnceCell;

use crate::prelude::*;

pub static VSOCK_DRIVER: SpinLock<OnceCell<VSockDriver>> = SpinLock::new(OnceCell::new());
// pub static VSOCK: VSock = VSock;

#[derive(Debug)]
pub struct VSock;

impl VSock {
    pub fn try_read(&self) -> Option<Box<[u8]>> {
        let mut lock = VSOCK_DRIVER.lock();
        let driver = lock.get_mut().unwrap();
        driver.read()
    }

    pub async fn read(&self) -> Box<[u8]> {
        loop {
            if let Some(result) = self.try_read() {
                return result;
            }
            crate::rt::yield_now().await;
        }
    }

    pub fn try_write(&self, data: Box<[u8]>) -> bool {
        let mut lock = VSOCK_DRIVER.lock();
        let driver = lock.get_mut().unwrap();
        driver.write(data)
    }

    pub async fn write(&self, data: Box<[u8]>) {
        loop {
            if self.try_write(data) {
                return;
            }
            crate::rt::yield_now().await;
            todo!("deal with multiple write attempts");
        }
    }
}

#[derive(Debug)]
pub struct VSockDriver {
    rx: ReceiveQueue,
    tx: TransmitQueue,
}

impl VSockDriver {
    pub unsafe fn new(value: common::vhost::VSockMetadata) -> Self {
        let rx = ReceiveQueue::new(VirtQueue::new(value.rx));
        let tx = TransmitQueue::new(VirtQueue::new(value.tx));
        Self { rx, tx }
    }

    pub fn read(&mut self) -> Option<Box<[u8]>> {
        self.rx.read()
    }

    pub fn write(&mut self, data: Box<[u8]>) -> bool {
        self.tx.write(data)
    }
}

#[derive(Debug)]
struct ReceiveQueue {
    q: VirtQueue,
}

impl ReceiveQueue {
    fn new_buf() -> Box<[u8]> {
        unsafe { Box::new_zeroed_slice(4096).assume_init() }
    }

    pub fn new(mut q: VirtQueue) -> Self {
        // Populate the RX queue with a bunch of write-only descriptors.
        unsafe {
            for i in 0..q.descriptors {
                let p = Box::into_raw(Self::new_buf());
                q.set_descriptor_mut(i, p);
                q.mark_available(i);
            }
        }
        q.notify();
        Self { q }
    }

    pub fn read(&mut self) -> Option<Box<[u8]>> {
        unsafe {
            let used = self.q.get_used()?;
            let i = used.id as usize;
            let p = self.q.get_descriptor(i);
            self.q.set_descriptor_mut(i, Box::into_raw(Self::new_buf()));
            let b = Box::from_raw(p);
            let mut v: Vec<u8> = b.into();
            v.truncate(used.len as usize); // TODO: modify allocator to avoid reallocating here
            Some(v.into())
        }
    }

    pub fn len(&self) -> usize {
        unsafe { self.q.used_len() }
    }

    #[allow(unused)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug)]
struct TransmitQueue {
    q: VirtQueue,
}

impl TransmitQueue {
    pub fn new(q: VirtQueue) -> Self {
        Self { q }
    }

    pub fn write(&mut self, data: Box<[u8]>) -> bool {
        self.discard_used();
        unsafe {
            let Some(i) = self.find_next_free_descriptor() else {
                return false;
            };
            log::info!("using descriptor {i} for write");
            self.q.set_descriptor(i, Box::into_raw(data));
            self.q.mark_available(i);
            self.q.notify();
        }
        true
    }

    fn find_next_free_descriptor(&self) -> Option<usize> {
        for i in 0..self.q.descriptors {
            unsafe {
                let desc = &*self.q.desc;
                if desc.0[i].addr == 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    fn discard_used(&mut self) {
        unsafe {
            while let Some(used) = self.q.get_used() {
                let i = used.id;
                log::info!("{i} was used");
                let p = self.q.get_descriptor(i as usize);
                let _ = Box::from_raw(p);
                self.q.clear_descriptor(i as usize);
            }
        }
    }
}

#[derive(Debug)]
struct VirtQueue {
    descriptors: usize,
    last_used: u16,
    desc: *mut DescriptorTable,
    used: *mut UsedRing,
    avail: *mut AvailableRing,
}

unsafe impl Send for VirtQueue {}

impl VirtQueue {
    unsafe fn new(value: common::vhost::VirtQueueMetadata) -> Self {
        let desc: (*mut (), usize) = (BuddyAllocator.from_offset(value.desc.0), value.desc.1);
        let used: (*mut (), usize) = (BuddyAllocator.from_offset(value.used.0), value.used.1);
        let avail: (*mut (), usize) = (BuddyAllocator.from_offset(value.avail.0), value.avail.1);
        let used: *mut UsedRing = core::ptr::from_raw_parts_mut(used.0, used.1);
        let last_used = (*used).idx;
        Self {
            descriptors: value.descriptors,
            last_used,
            desc: core::ptr::from_raw_parts_mut(desc.0, desc.1),
            used,
            avail: core::ptr::from_raw_parts_mut(avail.0, avail.1),
        }
    }
}

impl VirtQueue {
    pub unsafe fn set_descriptor(&mut self, i: usize, slice: *const [u8]) {
        let (addr, len) = slice.to_raw_parts();
        let addr = vm::ka2pa(addr);
        let desc = &mut *self.desc;
        desc.0[i] = Descriptor {
            addr: addr as u64,
            len: len as u32,
            flags: 0,
            next: 0,
        };
    }

    pub unsafe fn clear_descriptor(&mut self, i: usize) {
        let desc = &mut *self.desc;
        desc.0[i] = Descriptor {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        };
    }

    pub unsafe fn set_descriptor_mut(&mut self, i: usize, slice: *mut [u8]) {
        let (addr, len) = slice.to_raw_parts();
        let addr = vm::ka2pa(addr);
        let desc = &mut *self.desc;
        desc.0[i] = Descriptor {
            addr: addr as u64,
            len: len as u32,
            flags: 1 << 1, // write-only
            next: 0,
        };
    }

    pub unsafe fn get_descriptor(&self, i: usize) -> *mut [u8] {
        let desc = &mut *self.desc;
        let Descriptor {
            addr,
            len,
            flags: _flags,
            next: _,
        } = desc.0[i];
        let addr = vm::pa2ka(addr as usize);
        unsafe { core::slice::from_raw_parts_mut(addr, len as usize) }
    }

    pub unsafe fn mark_available(&mut self, i: usize) {
        let avail = &mut *self.avail;
        avail.ring[avail.idx as usize] = i as u16;
        avail.idx = avail.idx.wrapping_add(1);
    }

    pub unsafe fn get_used(&mut self) -> Option<UsedElement> {
        let used = &mut *self.used;
        if self.used_len() == 0 {
            return None;
        }
        let idx = self.last_used;
        let read = used.ring[idx as usize];
        self.last_used = idx.wrapping_add(1);
        Some(read)
    }

    pub unsafe fn used_len(&self) -> usize {
        let used = &mut *self.used;
        used.idx.wrapping_sub(self.last_used) as usize
    }

    pub fn notify(&mut self) {
        unsafe {
            crate::io::outb(0xf4, 0);
        }
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct DescriptorTable([Descriptor]);

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Descriptor {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Debug)]
pub struct UsedRing {
    flags: u16,
    idx: u16,
    ring: [UsedElement],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct UsedElement {
    id: u32,
    len: u32,
}

#[repr(C)]
pub struct AvailableRing {
    flags: u16,
    idx: u16,
    ring: [u16],
}

const _: () = const {
    assert!(core::mem::size_of::<Descriptor>() == 16);
    assert!(core::mem::size_of::<UsedElement>() == 8);
    assert!(core::mem::size_of::<u16>() == 2);
};
