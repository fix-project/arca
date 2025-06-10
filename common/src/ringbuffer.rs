use crate::refcnt::RefCnt;
use crate::sendable::Sendable;
use crate::BuddyAllocator;
use core::alloc::{Allocator, Layout};
use core::cell::SyncUnsafeCell;
use core::cmp::min;
use core::marker::PhantomData;
use core::mem::{ManuallyDrop, MaybeUninit};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

extern crate alloc;
use alloc::boxed::Box;
use snafu::Snafu;

#[repr(C)]
struct RingBuffer<'a> {
    read_hangup: AtomicBool,
    write_hangup: AtomicBool,
    read_counter: AtomicUsize,
    write_counter: AtomicUsize,
    allocator: &'a BuddyAllocator<'a>,
    buf: SyncUnsafeCell<[u8]>,
}

#[repr(C)]
pub struct Sender<'a, T: Sendable> {
    datatype: PhantomData<T>,
    valid: bool,
    rb: ManuallyDrop<RefCnt<'a, RingBuffer<'a>>>,
}

#[repr(C)]
pub struct Receiver<'a, T: Sendable> {
    datatype: PhantomData<T>,
    valid: bool,
    rb: ManuallyDrop<RefCnt<'a, RingBuffer<'a>>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Snafu)]
pub enum Error {
    WouldBlock,
    Disconnected,
}

pub type Result<T> = core::result::Result<T, Error>;

impl<'a> RingBuffer<'a> {
    fn new_in(
        capacity: usize,
        allocator: &'a BuddyAllocator<'a>,
    ) -> Box<RingBuffer<'a>, &'a BuddyAllocator<'a>> {
        unsafe {
            let layout = Layout::new::<AtomicBool>()
                .extend(Layout::new::<AtomicBool>())
                .unwrap()
                .0
                .extend(Layout::new::<AtomicUsize>())
                .unwrap()
                .0
                .extend(Layout::new::<AtomicUsize>())
                .unwrap()
                .0
                .extend(Layout::new::<&'a BuddyAllocator<'a>>())
                .unwrap()
                .0
                .extend(Layout::new::<u8>().repeat(capacity).unwrap().0)
                .unwrap()
                .0
                .pad_to_align();

            let p = allocator
                .allocate_zeroed(layout)
                .expect("failed to allocate");
            let p: *mut RingBuffer =
                core::ptr::from_raw_parts_mut(&raw mut (*p.as_ptr())[0], capacity);
            assert_eq!((*p).buf.get().len(), capacity);
            (*p).read_hangup = AtomicBool::new(false);
            (*p).write_hangup = AtomicBool::new(false);
            (*p).read_counter = AtomicUsize::new(0);
            (*p).write_counter = AtomicUsize::new(0);
            (*p).allocator = allocator;
            Box::from_raw_in(p, allocator)
        }
    }

    pub fn is_read_closed(&self) -> bool {
        self.read_hangup.load(Ordering::Acquire)
    }

    pub fn is_write_closed(&self) -> bool {
        self.write_hangup.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        let read_count = self.read_counter.load(Ordering::Acquire);
        let write_count = self.write_counter.load(Ordering::Acquire);
        read_count == write_count
    }

    pub fn is_full(&self) -> bool {
        let read_count = self.read_counter.load(Ordering::Acquire);
        let write_count = self.write_counter.load(Ordering::Acquire);
        (write_count + 1) % self.buf.get().len() == read_count
    }

    pub fn read(&self, buf: &mut [MaybeUninit<u8>]) -> Result<usize> {
        if self.write_hangup.load(Ordering::Acquire) {
            return Err(Error::Disconnected);
        }
        let len = buf.len();
        let read_count = self.read_counter.load(Ordering::Acquire);
        let write_count = self.write_counter.load(Ordering::Acquire);
        if read_count == write_count {
            return Err(Error::WouldBlock);
        }

        let mut end: usize;
        if write_count >= read_count {
            end = write_count;
        } else {
            end = self.buf.get().len();
        }

        end = min(end, read_count + len);

        let readable = unsafe { &(*self.buf.get())[read_count..end] };
        buf[..readable.len()].write_copy_of_slice(readable);
        self.read_counter
            .store(end % self.buf.get().len(), Ordering::Release);
        Ok(readable.len())
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        if self.read_hangup.load(Ordering::Acquire) {
            return Err(Error::Disconnected);
        }
        let len = buf.len();
        let read_count = self.read_counter.load(Ordering::Acquire);
        let write_count = self.write_counter.load(Ordering::Acquire);
        if (write_count + 1) % self.buf.get().len() == read_count {
            return Err(Error::WouldBlock);
        }

        let mut end: usize;

        // Write until the end of array or read_count - 1, up to len
        if read_count > write_count {
            end = read_count - 1;
        } else {
            end = self.buf.get().len();
            if read_count == 0 {
                end -= 1;
            }
        }

        end = min(end, write_count + len);

        let writable: &mut [u8] = unsafe { &mut (*self.buf.get())[write_count..end] };
        writable.copy_from_slice(&buf[..writable.len()]);

        self.write_counter
            .store(end % self.buf.get().len(), Ordering::Release);
        let result = writable.len();
        Ok(result)
    }

    pub fn read_exact(&self, mut buf: &mut [MaybeUninit<u8>]) -> Result<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &mut buf[n..];
                }
                Err(Error::WouldBlock) => {
                    #[cfg(feature = "std")]
                    {
                        std::thread::yield_now();
                    }
                    core::hint::spin_loop();
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    pub fn write_all(&self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(n) => buf = &buf[n..],
                Err(Error::WouldBlock) => {
                    #[cfg(feature = "std")]
                    {
                        std::thread::yield_now();
                    }
                    core::hint::spin_loop();
                }
                Err(e) => return Err(e),
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

impl<'a, T: Sendable> Sender<'a, T> {
    fn new(rb: RefCnt<'a, RingBuffer<'a>>) -> Self {
        Self {
            rb: ManuallyDrop::new(rb),
            valid: true,
            datatype: PhantomData,
        }
    }

    pub fn is_full(&self) -> bool {
        self.rb.is_full()
    }

    pub fn is_closed(&self) -> bool {
        self.rb.is_read_closed()
    }

    pub fn send(&mut self, data: T) -> Result<()> {
        unsafe {
            let slice = core::slice::from_raw_parts(
                &data as *const _ as *const u8,
                core::mem::size_of::<T>(),
            );
            self.rb.write_all(slice)
        }
    }

    pub fn try_send(&mut self, data: T) -> Result<()> {
        if self.is_closed() {
            Err(Error::Disconnected)
        } else if self.is_full() {
            Err(Error::WouldBlock)
        } else {
            self.send(data)
        }
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        self.rb.allocator()
    }

    pub fn into_raw_parts(mut self) -> (*mut (), usize) {
        unsafe {
            let inner = ManuallyDrop::take(&mut self.rb);
            self.valid = false;
            RefCnt::into_raw(inner).to_raw_parts()
        }
    }

    pub fn hangup(&self) {
        self.rb.write_hangup.store(true, Ordering::Release);
    }
}

impl<T: Sendable> Drop for Sender<'_, T> {
    fn drop(&mut self) {
        if self.valid {
            self.hangup();
            unsafe {
                ManuallyDrop::drop(&mut self.rb);
            }
        }
    }
}

impl<'a, T: Sendable> Receiver<'a, T> {
    fn new(rb: RefCnt<'a, RingBuffer<'a>>) -> Self {
        Self {
            rb: ManuallyDrop::new(rb),
            valid: true,
            datatype: PhantomData,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rb.is_empty()
    }

    pub fn is_closed(&self) -> bool {
        self.rb.is_write_closed()
    }

    pub fn recv(&mut self) -> Result<T> {
        unsafe {
            let mut x = MaybeUninit::uninit();
            let slice = x.as_bytes_mut();
            self.rb.read_exact(slice)?;
            Ok(x.assume_init())
        }
    }

    pub fn try_recv(&mut self) -> Result<T> {
        if self.is_closed() {
            Err(Error::Disconnected)
        } else if self.is_empty() {
            Err(Error::WouldBlock)
        } else {
            self.recv()
        }
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        self.rb.allocator()
    }

    pub fn into_raw_parts(mut self) -> (*mut (), usize) {
        unsafe {
            let inner = ManuallyDrop::take(&mut self.rb);
            self.valid = false;
            RefCnt::into_raw(inner).to_raw_parts()
        }
    }

    pub fn hangup(&self) {
        self.rb.read_hangup.store(true, Ordering::Release);
    }
}

impl<T: Sendable> Drop for Receiver<'_, T> {
    fn drop(&mut self) {
        if self.valid {
            self.hangup();
            unsafe {
                ManuallyDrop::drop(&mut self.rb);
            }
        }
    }
}

pub struct Endpoint<'a, S: Sendable, R: Sendable> {
    sender: Sender<'a, S>,
    receiver: Receiver<'a, R>,
}

#[repr(C)]
#[derive(Debug)]
pub struct EndpointRawData(usize, usize, usize, usize);

impl<'a, S: Sendable, R: Sendable> Endpoint<'a, S, R> {
    pub fn into_raw_parts(self, allocator: &BuddyAllocator) -> EndpointRawData {
        let sender_raw = self.sender.into_raw_parts();
        let receiver_raw = self.receiver.into_raw_parts();
        EndpointRawData(
            allocator.to_offset(sender_raw.0),
            sender_raw.1,
            allocator.to_offset(receiver_raw.0),
            receiver_raw.1,
        )
    }

    /// # Safety
    /// This function's inputs must describe a valid RingBufferEndPoint in the current address
    /// space, managed by the provided allocator.
    pub unsafe fn from_raw_parts(raw: &EndpointRawData, allocator: &'a BuddyAllocator) -> Self {
        let sender_ptr = core::ptr::from_raw_parts_mut(allocator.from_offset::<()>(raw.0), raw.1);
        let receiver_ptr = core::ptr::from_raw_parts_mut(allocator.from_offset::<()>(raw.2), raw.3);
        Self {
            sender: Sender::new(RefCnt::from_raw_in(sender_ptr, allocator)),
            receiver: Receiver::new(RefCnt::from_raw_in(receiver_ptr, allocator)),
        }
    }

    pub fn into_sender_receiver(self) -> (Sender<'a, S>, Receiver<'a, R>) {
        (self.sender, self.receiver)
    }

    pub fn allocator(&self) -> &'a BuddyAllocator<'a> {
        assert_eq!(
            self.sender.allocator() as *const _,
            self.receiver.allocator() as *const _
        );
        self.sender.allocator()
    }

    pub fn receiver_mut(&mut self) -> &mut Receiver<'a, R> {
        &mut self.receiver
    }

    pub fn sender_mut(&mut self) -> &mut Sender<'a, S> {
        &mut self.sender
    }
}

pub type RingBufferPair<'a, A, B> = (Endpoint<'a, A, B>, Endpoint<'a, B, A>);

fn channel<'a, T: Sendable>(
    capacity: usize,
    allocator: &'a BuddyAllocator<'a>,
) -> (Sender<'a, T>, Receiver<'a, T>) {
    let rb: RefCnt<'a, RingBuffer> = RingBuffer::new_in(capacity, allocator).into();
    (Sender::new(rb.clone()), Receiver::new(rb))
}

pub fn pair<'a, A: Sendable, B: Sendable>(
    capacity: usize,
    allocator: &'a BuddyAllocator<'a>,
) -> RingBufferPair<'a, A, B> {
    let (sender1, receiver1) = channel(capacity, allocator);
    let (sender2, receiver2) = channel(capacity, allocator);

    let endpoint1: Endpoint<A, B> = Endpoint {
        sender: sender1,
        receiver: receiver2,
    };
    let endpoint2: Endpoint<B, A> = Endpoint {
        sender: sender2,
        receiver: receiver1,
    };
    (endpoint1, endpoint2)
}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::Bencher;

    #[test]
    pub fn test_channel() {
        let mut region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let (mut tx, mut rx) = channel(1024, &allocator);
        let txbuf = 10u64;
        tx.send(txbuf).unwrap();
        let rxbuf = rx.recv().unwrap();
        assert_eq!(rxbuf, txbuf);
    }

    #[bench]
    pub fn bench_channel(b: &mut Bencher) {
        let mut region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        std::thread::scope(|s| {
            let (mut tx, mut rx) = channel(1024, &allocator);
            s.spawn(move || {
                let mut i = 0u64;
                loop {
                    if tx.send(i).is_err() {
                        return;
                    }
                    i = i.wrapping_add(1);
                }
            });
            let mut i: u64 = 0;
            b.iter(|| {
                let x = rx.recv().unwrap();
                assert_eq!(x, i);
                i = i.wrapping_add(1);
            });
        });
    }

    #[bench]
    pub fn bench_ping_pong(b: &mut Bencher) {
        let mut region: Box<[u8; 0x100000000]> = unsafe { Box::new_zeroed().assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        std::thread::scope(|s| {
            let (mut ep1, mut ep2) = pair(1024, &allocator);
            s.spawn(move || {
                let x = 0u64;
                let mut last: u64 = 0;
                ep2.sender_mut().send(x).unwrap();
                while let Ok(mut x) = ep2.receiver_mut().recv() {
                    assert_eq!(last.wrapping_add(1), x);
                    x = x.wrapping_add(1);
                    last = x;
                    let _ = ep2.sender_mut().send(x);
                }
            });
            b.iter(|| {
                let mut x = ep1.receiver_mut().recv().unwrap();
                x = x.wrapping_add(1);
                ep1.sender_mut().send(x).unwrap();
            });
        });
    }
}
