use crate::BuddyAllocator;
use core::alloc::{Allocator, Layout};
use core::cell::UnsafeCell;
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
pub struct RingBuffer<T: Clone> {
    read_counter: AtomicUsize,
    write_counter: AtomicUsize,
    buf: UnsafeCell<[T]>,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new_in<'a>(
        capacity: usize,
        allocator: &'a BuddyAllocator<'a>,
    ) -> Box<RingBuffer<T>, &'a BuddyAllocator<'a>> {
        unsafe {
            let layout = Layout::new::<AtomicUsize>()
                .extend(Layout::new::<AtomicUsize>())
                .unwrap()
                .0
                .extend(Layout::new::<T>().repeat(capacity).unwrap().0)
                .unwrap()
                .0
                .pad_to_align();

            let p = allocator
                .allocate_zeroed(layout)
                .expect("failed to allocate");
            let p: *mut RingBuffer<T> = core::mem::transmute(p);
            (*p).read_counter = AtomicUsize::new(0);
            (*p).write_counter = AtomicUsize::new(0);
            Box::from_raw_in(p, allocator)
        }
    }

    fn inc(&self, n: usize) -> usize {
        if n == self.buf.get().len() {
            0
        } else {
            n + 1
        }
    }

    pub fn into_raw_parts(&self, allocator: &BuddyAllocator) -> RingBufferRawData {
        let self_ptr: *const RingBuffer<T> = self;
        let (ptr, metadata) = self_ptr.to_raw_parts();
        RingBufferRawData(allocator.to_offset(ptr), metadata as usize)
    }

    pub unsafe fn from_raw_parts<'a>(
        raw: RingBufferRawData,
        allocator: &'a BuddyAllocator<'a>,
    ) -> Box<RingBuffer<T>, &'a BuddyAllocator<'a>> {
        let ptr =
            core::ptr::from_raw_parts_mut(allocator.from_offset::<()>(raw.0) as *mut (), raw.1);
        Box::from_raw_in(ptr, allocator)
    }

    pub fn try_read(&self) -> Result<T, RingBufferError> {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        if read_count == write_count {
            Err(RingBufferError::OOB(
                read_count,
                write_count,
                self.buf.get().len(),
            ))
        } else {
            let target = self.inc(read_count);

            match self.read_counter.compare_exchange(
                read_count,
                target,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => Ok(unsafe { (*self.buf.get())[read_count].clone() }),
                Err(_) => Err(RingBufferError::OOB(
                    read_count,
                    write_count,
                    self.buf.get().len(),
                )),
            }
        }
    }

    pub fn try_write(&self, val: &T) -> Result<usize, RingBufferError> {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);

        let target = self.inc(write_count);
        if target == read_count {
            Err(RingBufferError::OOB(
                read_count,
                write_count,
                self.buf.get().len(),
            ))
        } else {
            match self.write_counter.compare_exchange(
                write_count,
                target,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    unsafe { (*self.buf.get())[write_count] = val.clone() };
                    Ok(write_count)
                }
                Err(_) => Err(RingBufferError::OOB(
                    read_count,
                    write_count,
                    self.buf.get().len(),
                )),
            }
        }
    }

    pub fn read(&self) -> T {
        loop {
            match self.try_read() {
                Ok(res) => return res,
                Err(_) => {}
            }
        }
    }

    pub fn write(&self, val: T) -> () {
        loop {
            match self.try_write(&val) {
                Ok(_) => return {},
                Err(_) => {}
            }
        }
    }
}
