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

#[derive(Debug, Clone, Eq, PartialEq)]
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

    pub fn is_empty(&self) -> bool {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        read_count == write_count
    }

    pub fn is_full(&self) -> bool {
        let read_count = self.read_counter.load(Ordering::SeqCst);
        let write_count = self.write_counter.load(Ordering::SeqCst);
        (write_count + 1) % self.buf.get().len() == read_count
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

        self.read_counter
            .store(end % self.buf.get().len(), Ordering::SeqCst);
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

        self.write_counter
            .store(end % self.buf.get().len(), Ordering::SeqCst);
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

    pub fn is_full(&self) -> bool {
        self.rb.is_full()
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

    pub fn is_empty(&self) -> bool {
        self.rb.is_empty()
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

    pub fn receiver_mut(&mut self) -> &mut RingBufferReceiver<'a> {
        &mut self.receiver
    }

    pub fn sender_mut(&mut self) -> &mut RingBufferSender<'a> {
        &mut self.sender
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

#[cfg(test)]
mod tests {
    extern crate test;

    use core::sync::atomic::AtomicBool;
    use std::sync::Arc;

    use super::*;
    use test::Bencher;

    #[test]
    pub fn test_channel() {
        let mut region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let (mut tx, mut rx) = channel(1024, &allocator);
        let txbuf = 10u64.to_ne_bytes();
        let mut rxbuf = [0; 8];
        tx.write_all(&txbuf).unwrap();
        rx.read_exact(&mut rxbuf).unwrap();
        assert_eq!(u64::from_ne_bytes(rxbuf), 10);
    }

    #[bench]
    pub fn bench_channel(b: &mut Bencher) {
        let mut region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        std::thread::scope(|s| {
            let (mut tx, mut rx) = channel(1024, &allocator);
            let done = Arc::new(AtomicBool::new(false));
            let d2 = done.clone();
            s.spawn(move || {
                let mut i = 0;
                while !d2.load(Ordering::SeqCst) {
                    let x = [i; 1];
                    tx.write_all(&x).unwrap();
                    i = i.wrapping_add(1);
                }
                println!("exiting thread 2");
            });
            let mut i = 0;
            b.iter(|| {
                let mut x = [0; 1];
                rx.read_exact(&mut x).unwrap();
                assert_eq!(x[0], i);
                i = i.wrapping_add(1);
            });
            done.store(true, Ordering::SeqCst);
            let mut x = [0; 1];
            rx.read_exact(&mut x).unwrap();
        });
    }

    #[bench]
    pub fn bench_ping_pong(b: &mut Bencher) {
        let mut region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        std::thread::scope(|s| {
            let (mut ep1, mut ep2) = pair(1024, &allocator);
            s.spawn(move || {
                let mut x = [0; 2];
                let mut last: u8 = 0;
                ep2.sender_mut().write_all(&x).unwrap();
                loop {
                    ep2.receiver_mut().read_exact(&mut x).unwrap();
                    if x[1] == 1 {
                        return;
                    }
                    assert_eq!(last.wrapping_add(1), x[0]);
                    x[0] = x[0].wrapping_add(1);
                    last = x[0];
                    ep2.sender_mut().write_all(&x).unwrap();
                }
            });
            b.iter(|| {
                let mut x = [0; 2];
                ep1.receiver_mut().read_exact(&mut x).unwrap();
                x[0] = x[0].wrapping_add(1);
                ep1.sender_mut().write_all(&x).unwrap();
            });
            let x = [1; 2];
            ep1.sender_mut().write_all(&x).unwrap();
        });
    }
}
