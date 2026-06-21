extern crate alloc;
use alloc::sync::Arc;
use alloc::boxed::Box;
use core::alloc::Allocator;
use core::sync::atomic::Ordering;
use core::cell::SyncUnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicUsize};
use core::alloc::Layout;
use crate::BuddyAllocator;

use super::error::{Error, Result};

#[derive(Debug)]
#[repr(C)]
pub struct RingBuffer {
    read_cursor: AtomicUsize,
    write_cursor: AtomicUsize,
    writer_closed: AtomicBool,
    reader_closed: AtomicBool,
    buffer: SyncUnsafeCell<[u8]>,
}

impl RingBuffer {
    fn new(len: usize) -> Arc<RingBuffer, BuddyAllocator> {
        let layout = Layout::new::<AtomicUsize>()
            .extend(Layout::new::<AtomicUsize>()).unwrap().0
            .extend(Layout::new::<AtomicBool>()).unwrap().0
            .extend(Layout::new::<AtomicBool>()).unwrap().0
            .extend(Layout::new::<u8>().repeat(len).unwrap().0).unwrap().0
            .pad_to_align();
        let p: *mut u8 = BuddyAllocator
            .allocate_zeroed(layout)
            .expect("could not allocate shared memory for ringbuffer").as_mut_ptr();
        let (base, _) = p.to_raw_parts();
        let p: *mut RingBuffer = core::ptr::from_raw_parts_mut(base, len);
        unsafe {
            let b = Box::from_raw_in(p, BuddyAllocator);
            b.into()
        }
    }

    fn len(&self) -> usize {
        core::mem::size_of_val(&self.buffer)
    }

    unsafe fn readable_bytes(&self) -> *const [u8] {
        let start = self.read_cursor.load(Ordering::SeqCst) % self.len();
        let mut end = self.write_cursor.load(Ordering::SeqCst) % self.len();
        if end < start {
            end = self.len()
        }
        let len = end - start;
        let base = self.buffer.get() as *const u8;
        unsafe {
            core::ptr::slice_from_raw_parts(base.add(start), len)
        }
    }

    unsafe fn read(&self, bytes: usize) {
        let start = self.read_cursor.load(Ordering::SeqCst);
        let end = start + bytes;
        self.read_cursor.store(end, Ordering::SeqCst);
    }

    unsafe fn can_read(&self) -> bool {
        let read = self.read_cursor.load(Ordering::SeqCst);
        let write = self.write_cursor.load(Ordering::SeqCst);
        read != write
    }

    unsafe fn writer_open(&self) -> bool {
        !self.writer_closed.load(Ordering::SeqCst)
    }

    unsafe fn read_hangup(&self) {
        self.reader_closed.store(true, Ordering::SeqCst);
    }

    unsafe fn writeable_bytes(&self) -> *mut [u8] {
        let start = self.write_cursor.load(Ordering::SeqCst) % self.len();
        let mut end = (self.read_cursor.load(Ordering::SeqCst)  + self.len() - 1) % self.len();
        if end < start {
            end = self.len();
        }
        let len = end - start;
        let base = self.buffer.get() as *mut u8;
        unsafe {
            core::ptr::slice_from_raw_parts_mut(base.add(start), len)
        }
    }

    unsafe fn write(&self, bytes: usize) {
        let start = self.write_cursor.load(Ordering::SeqCst);
        let end = start + bytes % self.len();
        self.write_cursor.store(end, Ordering::SeqCst);
    }

    unsafe fn can_write(&self) -> bool {
        let read = self.read_cursor.load(Ordering::SeqCst);
        let write = self.write_cursor.load(Ordering::SeqCst);
        (read + self.len() - 1) % self.len() != write % self.len()
    }

    unsafe fn reader_open(&self) -> bool {
        !self.reader_closed.load(Ordering::SeqCst)
    }

    unsafe fn write_hangup(&self) {
        self.writer_closed.store(true, Ordering::SeqCst);
    }
}

pub fn channel(len: usize) -> (Reader, Writer) {
    let ring = RingBuffer::new(len);
    (Reader {
        ring: Some(ring.clone()),
    },
    Writer {
        ring: Some(ring)
    })
}

#[derive(Debug)]
pub struct Reader {
    ring: Option<Arc<RingBuffer, BuddyAllocator>>,
}

impl Reader {
    pub fn read(&mut self, data: &mut [u8]) -> Result<usize> {
        unsafe {
            if self.is_closed() {
                return Err(Error::Closed);
            }
            if !self.ring.as_ref().unwrap().can_read() {
                return Err(Error::WouldBlock);
            }
            let bytes = self.ring.as_ref().unwrap().readable_bytes();
            let len = core::cmp::min(data.len(), bytes.len());
            data[..len].copy_from_slice(&(&(*bytes))[..len]);
            self.ring.as_mut().unwrap().read(len);
            Ok(len)
        }
    }

    pub fn len(&self) -> usize {
        unsafe {
            self.ring.as_ref().unwrap().readable_bytes().len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_closed(&self) -> bool {
        unsafe {
            !self.ring.as_ref().unwrap().writer_open()
        }
    }

    pub fn into_inner(mut self) -> Arc<RingBuffer, BuddyAllocator> {
        self.ring.take().unwrap()
    }

    pub unsafe fn from_inner(ring: Arc<RingBuffer, BuddyAllocator>) -> Self {
        Self {
            ring: Some(ring)
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        if let Some(ring) = self.ring.as_mut() {
            unsafe {
                ring.read_hangup();
            }
        }
    }
}

#[derive(Debug)]
pub struct Writer {
    ring: Option<Arc<RingBuffer, BuddyAllocator>>,
}

impl Writer {
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        unsafe {
            if self.is_closed() {
                return Err(Error::Closed);
            }
            if !self.ring.as_ref().unwrap().can_write() {
                return Err(Error::WouldBlock);
            }
            let bytes = self.ring.as_ref().unwrap().writeable_bytes();
            let len = core::cmp::min(data.len(), bytes.len());
            (&mut (*bytes))[..len].copy_from_slice(&data[..len]);
            self.ring.as_ref().unwrap().write(len);
            Ok(len)
        }
    }

    pub fn len(&self) -> usize {
        unsafe {
            self.ring.as_ref().unwrap().writeable_bytes().len()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn is_closed(&self) -> bool {
        unsafe {
            !self.ring.as_ref().unwrap().reader_open()
        }
    }

    pub fn into_inner(mut self) -> Arc<RingBuffer, BuddyAllocator> {
        self.ring.take().unwrap()
    }

    pub unsafe fn from_inner(ring: Arc<RingBuffer, BuddyAllocator>) -> Self {
        Self {
            ring: Some(ring)
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if let Some(ring) = self.ring.as_mut() {
            unsafe {
                ring.write_hangup();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    pub fn test_send_recv() {
        let (mut rx, mut tx) = super::channel(1024);
        assert_eq!(tx.len(), 1023);
        assert_eq!(rx.len(), 0);

        let len = tx.write(b"hello");
        assert_eq!(len, Ok(5));
        assert_eq!(tx.len(), 1018);
        assert_eq!(rx.len(), 5);

        let mut buf = [0; 5];
        let len = rx.read(&mut buf);
        assert_eq!(len, Ok(5));
        assert_eq!(rx.len(), 0);
        assert_eq!(&buf, b"hello");
    }

    #[test]
    pub fn test_send_recv_wrapping() {
        let (mut rx, mut tx) = super::channel(4);
        assert_eq!(tx.len(), 3);
        assert_eq!(rx.len(), 0);

        let len = tx.write(b"hello");
        assert_eq!(len, Ok(3));
        assert_eq!(tx.len(), 0);
        assert_eq!(rx.len(), 3);

        let mut buf = [0; 3];
        let len = rx.read(&mut buf);
        assert_eq!(len, Ok(3));
        assert_eq!(rx.len(), 0);
        assert_eq!(tx.len(), 1);
        assert_eq!(&buf, b"hel");

        let len = tx.write(b"lo");
        assert_eq!(len, Ok(1));
        assert_eq!(tx.len(), 2);
        assert_eq!(rx.len(), 1);
    }

    #[test]
    pub fn test_would_block() {
        let (mut rx, mut tx) = super::channel(4);

        tx.write(b"xx").unwrap();
        let mut buf = [0; 2];
        rx.read(&mut buf).unwrap();

        let len = tx.write(b"yyzz");
        assert_eq!(len, Ok(2));
        let len = tx.write(b"zz");
        assert_eq!(len, Ok(1));
        let len = tx.write(b"z");
        assert_eq!(len, Err(Error::WouldBlock));
    }
}
