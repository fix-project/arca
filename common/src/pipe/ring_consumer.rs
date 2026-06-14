use crate::pipe::error::PipeError;
use crate::pipe::ring::{RingData, RingHeader};
use crate::pipe::shared_memory_region::SharedMemoryRegion;
use crate::pipe::traits;
use core::sync::atomic::Ordering;

/// Consumer (read) end of a single SPSC ring buffer.
///
/// Like [`RingProducer`](crate::pipe::ring_producer::RingProducer), it owns its
/// own clone of the [`SharedMemoryRegion`] handle and holds a raw header pointer
/// rather than a borrow — so it carries no lifetime, keeps the mapping alive
/// independently, and is a self-contained `Send` value returned by
/// [`BidirectionalPipe::split`](crate::pipe::bidirectional_pipe::BidirectionalPipe::split).
///
/// [`SharedMemoryRegion`]: crate::pipe::shared_memory_region::SharedMemoryRegion
pub struct RingConsumer<R: SharedMemoryRegion> {
    /// Liveness only: keeps the mapped bytes alive for as long as this end exists.
    ///
    /// Drop order is safe regardless of field position: `header`/`data` are raw
    /// pointers with no-op drops that never dereference the region, so dropping
    /// `region` (which may unmap the memory for an mmap-backed `R`) cannot cause
    /// a use-after-free.
    #[allow(dead_code)]
    region: R,
    header: *const RingHeader,
    data: RingData,
}

// SAFETY: the consumer owns the sole read cursor of an SPSC ring. The raw
// pointers address shared memory kept alive by the owned region handle; SPSC
// discipline guarantees a single consumer, so moving it to another thread
// (when `R: Send`) cannot introduce a data race.
unsafe impl<R: SharedMemoryRegion + Send> Send for RingConsumer<R> {}

impl<R: SharedMemoryRegion> RingConsumer<R> {
    pub fn new(region: R, header: *const RingHeader, data: RingData) -> Self {
        Self {
            region,
            header,
            data,
        }
    }

    /// Access the ring header living in shared memory.
    fn header(&self) -> &RingHeader {
        // SAFETY: pointer derived from a valid, aligned shared region; valid
        // for the consumer's lifetime.
        unsafe { &*self.header }
    }

    /// Signal that this consumer will read no more bytes.
    ///
    /// Takes `&mut self`: there is exactly one consumer, so closing is an
    /// exclusive operation and cannot race with a concurrent read on this end.
    pub fn close_reader(&mut self) {
        self.header().reader_closed.store(true, Ordering::Release);
    }

    /// True if the producer has closed its write end.
    pub fn is_writer_closed(&self) -> bool {
        self.header().writer_closed.load(Ordering::Acquire)
    }

    /// True when both ends of this ring are closed.
    pub fn is_closed(&self) -> bool {
        self.header().is_closed()
    }
}

impl<R: SharedMemoryRegion> traits::Read for RingConsumer<R> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Observe `writer_closed` BEFORE the cursors. If the writer has closed,
        // its final `write_cursor` store happened-before the close store
        // (Acquire here synchronizes-with that Release), so the `readable_len`
        // load below is guaranteed to see every byte written. This prevents
        // reporting EOF while unread bytes are still in flight.
        let writer_closed = self.header().writer_closed.load(Ordering::Acquire);
        let used = self.header().readable_len();
        if used == 0 {
            // Empty: EOF once the writer is done, otherwise nothing yet.
            return if writer_closed {
                Ok(0)
            } else {
                Err(PipeError::WouldBlock)
            };
        }

        let n = core::cmp::min(buf.len() as u64, used) as usize;
        let cursor = self.header().read_cursor.load(Ordering::Relaxed);
        self.data.read_at(cursor, &mut buf[..n]);

        // No standalone fence needed, release on the store guarantees the
        // preceding read_at is visible before the cursor update
        self.header()
            .read_cursor
            .store(cursor + n as u64, Ordering::Release);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::shared_memory_region::RawSharedMemoryRegion;
    use crate::pipe::traits::Read;
    use core::sync::atomic::AtomicU64;

    /// Region handle for tests — ownership/liveness only; the ring's actual
    /// header/data pointers are supplied separately below.
    fn raw(mem: &mut [u8]) -> RawSharedMemoryRegion {
        unsafe { RawSharedMemoryRegion::from_raw(mem.as_mut_ptr(), mem.len() as u64) }
    }

    fn header(read: u64, write: u64) -> RingHeader {
        use core::sync::atomic::AtomicBool;
        RingHeader {
            read_cursor: AtomicU64::new(read),
            write_cursor: AtomicU64::new(write),
            writer_closed: AtomicBool::new(false),
            reader_closed: AtomicBool::new(false),
        }
    }

    #[test]
    fn simple_read() {
        let mut mem = *b"hello...";
        let h = header(0, 5);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 8];
        assert_eq!(c.read(&mut out).unwrap(), 5);
        assert_eq!(&out[..5], b"hello");
        assert_eq!(h.read_cursor.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn partial_read() {
        let mut mem = *b"abcdefgh";
        let h = header(0, 8);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 3];
        assert_eq!(c.read(&mut out).unwrap(), 3);
        assert_eq!(&out, b"abc");
    }

    #[test]
    fn wrap_around() {
        let mut mem = *b"WXYZabcd";
        let h = header(5, 12);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 8];
        assert_eq!(c.read(&mut out).unwrap(), 7);
        assert_eq!(&out[..7], b"bcdWXYZ");
    }

    #[test]
    fn empty_ring_blocks() {
        let mut mem = [0u8; 4];
        let h = header(4, 4);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 4];
        assert!(matches!(c.read(&mut out), Err(PipeError::WouldBlock)));
    }

    #[test]
    fn eof_when_writer_closed_and_empty() {
        let mut mem = [0u8; 4];
        let h = header(0, 0);
        h.writer_closed.store(true, Ordering::Release);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 4];
        // Drained + writer closed = EOF, reported as Ok(0) (not WouldBlock).
        assert_eq!(c.read(&mut out).unwrap(), 0);
    }

    #[test]
    fn drains_remaining_then_eof() {
        let mut mem = *b"hi..";
        let h = header(0, 2);
        h.writer_closed.store(true, Ordering::Release);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 4];
        // Pending bytes are still delivered even though the writer has closed...
        assert_eq!(c.read(&mut out).unwrap(), 2);
        assert_eq!(&out[..2], b"hi");
        // ...and only then is EOF reported.
        assert_eq!(c.read(&mut out).unwrap(), 0);
    }

    #[test]
    fn zero_length_read_non_empty() {
        let mut mem = *b"data";
        let h = header(0, 4);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 0];
        assert_eq!(c.read(&mut out).unwrap(), 0);
        assert_eq!(h.read_cursor.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn zero_length_read_empty() {
        let mut mem = [0u8; 4];
        let h = header(0, 0);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(raw(&mut mem), &h, data);
        let mut out = [0u8; 0];
        assert_eq!(c.read(&mut out).unwrap(), 0);
    }
}
