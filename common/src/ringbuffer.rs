use crate::refcnt::RefCnt;
use crate::BuddyAllocator;
use core::alloc::{Allocator, Layout};
use core::cell::SyncUnsafeCell;
use core::cmp::min;
use core::sync::atomic::{AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;

#[repr(C)]
struct RingBuffer<'a> {
    read_counter: AtomicUsize,
    write_counter: AtomicUsize,
    allocator: &'a BuddyAllocator<'a>,
    buf: SyncUnsafeCell<[u8]>,
}

#[repr(C)]
pub struct RingBufferSender<'a> {
    rb: RefCnt<'a, RingBuffer<'a>>,
}

#[repr(C)]
pub struct RingBufferReceiver<'a> {
    rb: RefCnt<'a, RingBuffer<'a>>,
}

#[derive(Debug, Clone)]
pub enum RingBufferError {
    WouldBlock,
    ParseError,
    TypeError,
}

impl<'a> RingBuffer<'a> {
    fn new_in(
        capacity: usize,
        allocator: &'a BuddyAllocator<'a>,
    ) -> Box<RingBuffer<'a>, &'a BuddyAllocator<'a>> {
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
            (*p).allocator = allocator;
            Box::from_raw_in(p, allocator)
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize, RingBufferError> {
        let len = buf.len();
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        if read_count == write_count {
            return Err(RingBufferError::WouldBlock);
        }

        let mut end: usize;
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
            Err(_) => panic!("Read count changed while writing"),
        }

        let readable = unsafe { &(*self.buf.get())[read_count..end] };
        buf[..readable.len()].copy_from_slice(readable);
        Ok(readable.len())
    }

    fn write(&self, buf: &[u8]) -> Result<usize, RingBufferError> {
        let len = buf.len();
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        if (write_count + 1) % self.buf.get().len() == read_count {
            return Err(RingBufferError::WouldBlock);
        }

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
        Ok(result)
    }

    fn read_exact(&self, mut buf: &mut [u8]) -> Result<(), RingBufferError> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    core::hint::spin_loop();
                    buf = &mut buf[n..];
                }
                Err(_) => {}
            }
        }
        Ok(())
    }

    fn write_all(&self, mut buf: &[u8]) -> Result<(), RingBufferError> {
        while !buf.is_empty() {
            if let Ok(n) = self.write(buf) {
                core::hint::spin_loop();
                buf = &buf[n..]
            }
        }
        Ok(())
    }

    pub fn allocator<'b>(&self) -> &'b BuddyAllocator<'a>
    where
        'a: 'b,
    {
        self.allocator
    }
}

impl<'a> RingBufferSender<'a> {
    fn new(rb: RefCnt<'a, RingBuffer<'a>>) -> Self {
        Self { rb }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize, RingBufferError> {
        self.rb.write(buf)
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<(), RingBufferError> {
        self.rb.write_all(buf)
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        self.rb.allocator()
    }
}

impl<'a> RingBufferReceiver<'a> {
    fn new(rb: RefCnt<'a, RingBuffer<'a>>) -> Self {
        Self { rb }
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, RingBufferError> {
        self.rb.read(buf)
    }

    pub fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), RingBufferError> {
        self.rb.read_exact(buf)
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        self.rb.allocator()
    }
}

pub struct RingBufferEndPoint<'a> {
    sender: RingBufferSender<'a>,
    receiver: RingBufferReceiver<'a>,
}

#[repr(C)]
#[derive(Debug)]
pub struct RingBufferEndPointRawData(usize, usize, usize, usize);

impl<'a> RingBufferEndPoint<'a> {
    pub fn into_raw_parts(self, allocator: &BuddyAllocator) -> RingBufferEndPointRawData {
        let sender_raw = RefCnt::into_raw(self.sender.rb).to_raw_parts();
        let receiver_raw = RefCnt::into_raw(self.receiver.rb).to_raw_parts();
        RingBufferEndPointRawData(
            allocator.to_offset(sender_raw.0),
            sender_raw.1,
            allocator.to_offset(receiver_raw.0),
            receiver_raw.1,
        )
    }

    /// # Safety
    /// This function's inputs must describe a valid RingBufferEndPoint in the current address
    /// space, managed by the provided allocator.
    pub unsafe fn from_raw_parts(
        raw: &RingBufferEndPointRawData,
        allocator: &'a BuddyAllocator,
    ) -> Self {
        let sender_ptr = core::ptr::from_raw_parts_mut(allocator.from_offset::<()>(raw.0), raw.1);
        let receiver_ptr = core::ptr::from_raw_parts_mut(allocator.from_offset::<()>(raw.2), raw.3);
        Self {
            sender: RingBufferSender::new(RefCnt::from_raw_in(sender_ptr, allocator)),
            receiver: RingBufferReceiver::new(RefCnt::from_raw_in(receiver_ptr, allocator)),
        }
    }

    pub fn into_sender_receiver(self) -> (RingBufferSender<'a>, RingBufferReceiver<'a>) {
        (self.sender, self.receiver)
    }

    pub fn allocator(&self) -> &BuddyAllocator {
        assert_eq!(
            self.sender.allocator() as *const _,
            self.receiver.allocator() as *const _
        );
        self.sender.allocator()
    }
}

pub type RingBufferPair<'a> = (RingBufferEndPoint<'a>, RingBufferEndPoint<'a>);

fn channel<'a>(
    capacity: usize,
    allocator: &'a BuddyAllocator<'a>,
) -> (RingBufferSender<'a>, RingBufferReceiver<'a>) {
    let rb: RefCnt<'a, RingBuffer> = RingBuffer::new_in(capacity, allocator).into();
    (
        RingBufferSender::new(rb.clone()),
        RingBufferReceiver::new(rb),
    )
}

pub fn pair<'a>(capacity: usize, allocator: &'a BuddyAllocator<'a>) -> RingBufferPair<'a> {
    let (sender1, receiver1) = channel(capacity, allocator);
    let (sender2, receiver2) = channel(capacity, allocator);

    let endpoint1: RingBufferEndPoint = RingBufferEndPoint {
        sender: sender1,
        receiver: receiver2,
    };
    let endpoint2: RingBufferEndPoint = RingBufferEndPoint {
        sender: sender2,
        receiver: receiver1,
    };
    (endpoint1, endpoint2)
}
