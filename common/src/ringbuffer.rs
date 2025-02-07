use crate::BuddyAllocator;
use core::alloc::{Allocator, Layout};
use core::cell::SyncUnsafeCell;
use core::cmp::min;
use core::sync::atomic::{AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use core::clone::Clone;

pub enum RingBufferError {
    OOB(usize, usize, usize),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RingBufferRawData(usize, usize);

#[repr(C)]
pub struct RingBuffer {
    read_counter: AtomicUsize,
    write_counter: AtomicUsize,
    buf: SyncUnsafeCell<[u8]>,
}

impl RingBuffer {
    pub fn new_in<'a>(
        capacity: usize,
        allocator: &'a BuddyAllocator<'a>,
    ) -> Box<RingBuffer, &'a BuddyAllocator<'a>> {
        unsafe {
            let layout = Layout::new::<AtomicUsize>()
                .extend(Layout::new::<AtomicUsize>())
                .unwrap()
                .0
                .extend(Layout::new::<u8>().repeat(capacity).unwrap().0)
                .unwrap()
                .0
                .pad_to_align();

            let p = allocator
                .allocate_zeroed(layout)
                .expect("failed to allocate");
            let p: *mut RingBuffer = core::mem::transmute(p);
            (*p).read_counter = AtomicUsize::new(0);
            (*p).write_counter = AtomicUsize::new(0);
            Box::from_raw_in(p, allocator)
        }
    }

    pub fn into_raw_parts(&self, allocator: &BuddyAllocator) -> RingBufferRawData {
        let self_ptr: *const RingBuffer = self;
        let (ptr, metadata) = self_ptr.to_raw_parts();
        RingBufferRawData(allocator.to_offset(ptr), metadata as usize)
    }

    pub unsafe fn from_raw_parts<'a>(
        raw: RingBufferRawData,
        allocator: &'a BuddyAllocator<'a>,
    ) -> Box<RingBuffer, &'a BuddyAllocator<'a>> {
        let ptr =
            core::ptr::from_raw_parts_mut(allocator.from_offset::<()>(raw.0) as *mut (), raw.1);
        Box::from_raw_in(ptr, allocator)
    }

    fn readable_region(&self, len: usize) -> &[u8] {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        let mut end: usize;

        // Read until the end of array or write_count, up to len
        if write_count >= read_count {
            end = write_count;
        } else {
            end = self.buf.get().len();
        }

        end = min(end, read_count + len);

        match self.read_counter.compare_exchange(
            read_count,
            end % self.buf.get().len(),
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {}
            Err(_) => end = read_count,
        }

        unsafe { &(*self.buf.get())[read_count..end] }
    }

    pub fn try_read(&self, buf: &mut [u8]) -> usize {
        let readable = self.readable_region(buf.len());
        buf[..readable.len()].copy_from_slice(&readable);
        readable.len()
    }

    pub fn try_write(&self, buf: &[u8]) -> usize {
        let len = buf.len();
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        let mut end: usize;

        // Write until the end of array or read_count - 1, up to len
        if read_count > write_count {
            end = read_count - 1;
        } else {
            end = self.buf.get().len();
        }

        end = min(end, write_count + len);

        let writable: &mut [u8] = unsafe { &mut (*self.buf.get())[write_count..end] };
        writable.copy_from_slice(&buf[..writable.len()]);

        match self.write_counter.compare_exchange(
            write_count,
            end % self.buf.get().len(),
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => {}
            Err(_) => panic!("Write count changed while writing"),
        }

        let result = writable.len();
        result
    }

    pub fn read(&self, buf: &mut [u8]) -> () {
        let mut offset = 0;
        while offset < buf.len() {
            let res = self.try_read(&mut buf[offset..]);
            offset += res;
        }
    }

    pub fn write(&self, buf: &[u8]) -> () {
        let mut offset = 0;
        while offset < buf.len() {
            let res = self.try_write(&buf[offset..]);
            offset += res;
        }
    }

    pub fn readable(&self) -> bool {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        read_count != write_count
    }

    pub fn writable(&self) -> bool {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        (write_count + 1) % self.buf.get().len() != read_count
    }
}
