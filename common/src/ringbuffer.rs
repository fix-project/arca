use crate::BuddyAllocator;
use core::sync::atomic::{AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::clone::Clone;

pub enum RingBufferError {
    OOB(usize, usize, usize),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RingBufferRawData {
    pub read_counter_offset: usize,
    pub write_counter_offset: usize,
    pub buf_offset: usize,
    pub buf_size: usize,
}

#[repr(C)]
pub struct RingBuffer<'a, T: Clone> {
    read_counter: &'a mut AtomicUsize,
    write_counter: &'a mut AtomicUsize,
    buf: &'a mut [Option<T>],
}

impl<T: Clone> RingBuffer<'_, T> {
    pub unsafe fn from_raw_parts(raw: RingBufferRawData, allocator: &BuddyAllocator) -> Self {
        let RingBufferRawData {
            read_counter_offset,
            write_counter_offset,
            buf_offset,
            buf_size,
        } = raw;

        let read_counter_ptr: *const AtomicUsize = allocator.from_offset(read_counter_offset);
        let write_counter_ptr: *const AtomicUsize = allocator.from_offset(write_counter_offset);
        let buf_ptr: *const () = allocator.from_offset(buf_offset);
        let buf = core::ptr::from_raw_parts_mut(buf_ptr as *mut (), buf_size);

        RingBuffer {
            read_counter: &mut *(read_counter_ptr as *mut AtomicUsize),
            write_counter: &mut *(write_counter_ptr as *mut AtomicUsize),
            buf: &mut *buf,
        }
    }

    pub fn into_raw_parts(self, allocator: &BuddyAllocator) -> RingBufferRawData {
        let RingBuffer {
            read_counter,
            write_counter,
            buf,
        } = self;

        let (buf_offset, buf_size) = (buf as *const [Option<T>]).to_raw_parts();

        RingBufferRawData {
            read_counter_offset: allocator.to_offset(read_counter),
            write_counter_offset: allocator.to_offset(write_counter),
            buf_offset: allocator.to_offset(buf_offset),
            buf_size,
        }
    }

    pub fn with_capacity(capacity: usize, allocator: &BuddyAllocator) -> RingBufferRawData {
        let mut read_counter = Box::new_in(AtomicUsize::new(0), allocator);
        let mut write_counter = Box::new_in(AtomicUsize::new(0), allocator);
        let mut buf: Vec<Option<T>, &&BuddyAllocator<'_>> =
            Vec::with_capacity_in(capacity, &allocator);
        buf.resize(capacity, None);

        let res = RingBuffer {
            read_counter: &mut *read_counter,
            write_counter: &mut *write_counter,
            buf: &mut buf,
        }
        .into_raw_parts(allocator);

        Box::leak(read_counter);
        Box::leak(write_counter);
        Vec::leak(buf);

        res
    }

    fn inc(&self, n: usize) -> usize {
        if n == self.buf.len() {
            0
        } else {
            n + 1
        }
    }

    pub fn try_read(&mut self) -> Result<T, RingBufferError> {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        if read_count == write_count {
            Err(RingBufferError::OOB(
                read_count,
                write_count,
                self.buf.len(),
            ))
        } else {
            let target = self.inc(read_count);

            match self.read_counter.compare_exchange(
                read_count,
                target,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => Ok(self.buf[read_count].take().unwrap_or_else(|| {
                    panic!(
                        "empty-slot {} {} {}",
                        read_count,
                        write_count,
                        self.buf.len()
                    )
                })),
                Err(_) => Err(RingBufferError::OOB(
                    read_count,
                    write_count,
                    self.buf.len(),
                )),
            }
        }
    }

    pub fn try_write(&mut self, val: &T) -> Result<usize, RingBufferError> {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);

        let target = self.inc(write_count);
        if target == read_count {
            Err(RingBufferError::OOB(
                read_count,
                write_count,
                self.buf.len(),
            ))
        } else {
            match self.write_counter.compare_exchange(
                write_count,
                target,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    self.buf[write_count] = Some(val.clone());
                    Ok(write_count)
                }
                Err(_) => Err(RingBufferError::OOB(
                    read_count,
                    write_count,
                    self.buf.len(),
                )),
            }
        }
    }

    pub fn read(&mut self) -> T {
        loop {
            match self.try_read() {
                Ok(res) => return res,
                Err(_) => {}
            }
        }
    }

    pub fn write(&mut self, val: T) -> () {
        loop {
            match self.try_write(&val) {
                Ok(_) => return {},
                Err(_) => {}
            }
        }
    }
}

impl RingBufferRawData {
    pub fn to_kernel_offset(self, allocator: &BuddyAllocator) -> usize {
        let b = Box::new_in(self, allocator);
        let res = allocator.to_offset(&*b);
        Box::leak(b);
        res
    }
}
