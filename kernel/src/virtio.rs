use crate::prelude::*;

pub mod vsock;

#[derive(Debug)]
struct ReceiveQueue {
    q: SpinLock<VirtQueue>,
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
        Self {
            q: SpinLock::new(q),
        }
    }

    pub fn try_recv(&self) -> Option<Box<[u8]>> {
        let mut q = self.q.lock();
        unsafe {
            let Some(used) = q.get_used() else {
                return None;
            };
            let i = used.id as usize;
            let p = q.get_descriptor(i);
            let buf = Box::from_raw(p as *mut [u8]);
            q.set_descriptor(i, Box::into_raw(Self::new_buf()));
            q.mark_available(i);
            q.notify();
            let mut v: Vec<u8> = buf.into();
            v.truncate(used.len as usize);
            Some(v.into())
        }
    }
}

#[derive(Debug)]
struct TransmitQueue {
    add_free_descriptor: channel::Sender<usize>,
    get_free_descriptor: channel::Receiver<usize>,
    q: SpinLock<VirtQueue>,
}

unsafe impl Send for TransmitQueue {}

impl TransmitQueue {
    pub fn new(q: VirtQueue) -> Self {
        let (tx, rx) = channel::unbounded();
        for i in 0..q.descriptors {
            tx.send_blocking(i).unwrap();
        }
        Self {
            add_free_descriptor: tx,
            get_free_descriptor: rx,
            q: SpinLock::new(q),
        }
    }

    pub fn send(&self, bufs: &[*const [u8]]) {
        let mut q = self.q.lock();
        let mut descs = [0; 16];
        assert!(bufs.len() <= descs.len());
        for (i, buf) in bufs.iter().enumerate() {
            let d = self.get_free_descriptor.try_recv().unwrap().unwrap();
            unsafe {
                q.set_descriptor(d, *buf);
                if i >= 1 {
                    q.link_descriptor(descs[i - 1], d);
                }
            }
            descs[i] = d;
        }
        unsafe {
            for i in 0..bufs.len() {
                q.mark_available(descs[i]);
            }
            q.notify();
        }
    }

    pub fn get_used(&self, mut callback: impl FnMut(*const [u8])) {
        let mut q = self.q.lock();
        unsafe {
            while let Some(used) = q.get_used() {
                let mut idx = Some(used.id as usize);
                while let Some(i) = idx {
                    let p = q.get_descriptor(i as usize);
                    callback(p);
                    idx = q.get_next(i as usize);
                    q.clear_descriptor(i as usize);
                    self.add_free_descriptor.send_blocking(i as usize).unwrap();
                }
            }
        }
    }
}

#[derive(Debug)]
struct VirtQueue {
    descriptors: usize,
    last_used: u16,
    buf_alloc: usize,
    fwd_cnt: usize,
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
            buf_alloc: 0,
            fwd_cnt: 0,
            desc: core::ptr::from_raw_parts_mut(desc.0, desc.1),
            used,
            avail: core::ptr::from_raw_parts_mut(avail.0, avail.1),
        }
    }

    pub unsafe fn set_descriptor(&mut self, i: usize, slice: *const [u8]) {
        self.clear_descriptor(i);
        let (addr, len) = slice.to_raw_parts();
        let addr = vm::ka2pa(addr);
        let desc = &mut *self.desc;
        desc.0[i] = Descriptor {
            addr: addr as u64,
            len: len as u32,
            flags: 0,
            next: 0,
        };
        self.buf_alloc += len as usize;
    }

    pub unsafe fn link_descriptor(&mut self, i: usize, j: usize) {
        let desc = &mut *self.desc;
        desc.0[i].flags |= 1;
        desc.0[i].next = j as u16;
    }

    pub unsafe fn clear_descriptor(&mut self, i: usize) {
        let desc = &mut *self.desc;
        let len = desc.0[i].len;
        desc.0[i] = Descriptor {
            addr: 0,
            len: 0,
            flags: 0,
            next: 0,
        };
        self.buf_alloc -= len as usize;
    }

    pub unsafe fn set_descriptor_mut(&mut self, i: usize, slice: *mut [u8]) {
        self.clear_descriptor(i);
        let (addr, len) = slice.to_raw_parts();
        let addr = vm::ka2pa(addr);
        let desc = &mut *self.desc;
        desc.0[i] = Descriptor {
            addr: addr as u64,
            len: len as u32,
            flags: 1 << 1, // write-only
            next: 0,
        };
        self.buf_alloc += len as usize;
    }

    pub unsafe fn get_descriptor(&self, i: usize) -> *const [u8] {
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

    pub unsafe fn get_next(&self, i: usize) -> Option<usize> {
        let desc = &mut *self.desc;
        if desc.0[i].flags & 1 == 1 {
            Some(desc.0[i].next as usize)
        } else {
            None
        }
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
        // self.fwd_cnt = self.fwd_cnt.wrapping_add(read.len as usize);
        self.fwd_cnt = 0;
        Some(read)
    }

    pub unsafe fn used_len(&self) -> usize {
        let used = &mut *self.used;
        let idx = &raw const used.idx;
        let idx = idx.read_volatile();
        idx.wrapping_sub(self.last_used) as usize
    }

    pub fn notify(&mut self) {
        unsafe {
            crate::io::outb(0xf4, 0);
        }
    }

    pub fn buf_alloc(&self) -> u32 {
        self.buf_alloc as u32
    }

    pub fn fwd_cnt(&self) -> u32 {
        self.fwd_cnt as u32
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
