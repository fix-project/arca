use crate::pipe::error::PipeError;
use crate::pipe::ring::{RingData, RingHeader};
use crate::pipe::shared_memory_region::SharedMemoryRegion;
use crate::pipe::traits;
use core::sync::atomic::Ordering;

/// Producer (write) end of a single SPSC ring buffer.
///
/// Owns its own clone of the [`SharedMemoryRegion`] handle, so it keeps the
/// shared mapping alive independently and carries no lifetime. Combined with
/// the raw header pointer (rather than a borrow), this makes the producer a
/// self-contained, movable, `Send` value — exactly what
/// [`BidirectionalPipe::split`] hands back.
///
/// [`SharedMemoryRegion`]: crate::pipe::shared_memory_region::SharedMemoryRegion
/// [`BidirectionalPipe::split`]: crate::pipe::bidirectional_pipe::BidirectionalPipe::split
pub struct RingProducer<R: SharedMemoryRegion> {
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

// SAFETY: the producer owns the sole write cursor of an SPSC ring. The raw
// pointers address shared memory whose liveness is guaranteed by the owned
// region handle; SPSC discipline guarantees there is never more than one
// producer, so moving it to another thread (when `R: Send`) cannot introduce a
// data race.
unsafe impl<R: SharedMemoryRegion + Send> Send for RingProducer<R> {}

impl<R: SharedMemoryRegion> RingProducer<R> {
    pub fn new(region: R, header: *const RingHeader, data: RingData) -> Self {
        Self {
            region,
            header,
            data,
        }
    }

    /// Access the ring header living in shared memory.
    ///
    /// Borrows are deliberately kept short-lived (never held across a mutation
    /// of `data`) so this stays sound with `&mut self` ring operations.
    fn header(&self) -> &RingHeader {
        // SAFETY: see the struct docs — the pointer was derived from a valid,
        // correctly-aligned shared region and stays valid for our lifetime.
        unsafe { &*self.header }
    }

    /// Bytes written by this producer that the consumer has not yet read.
    /// Uses Acquire on read_cursor so this can be called cross-thread safely.
    pub fn bytes_pending(&self) -> u64 {
        let header = self.header();
        let write = header.write_cursor.load(Ordering::Relaxed);
        let read = header.read_cursor.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Signal that this producer will write no more bytes.
    ///
    /// Takes `&mut self`: there is exactly one producer, so closing is an
    /// exclusive operation and cannot race with a concurrent write on this end.
    pub fn close_writer(&mut self) {
        self.header().writer_closed.store(true, Ordering::Release);
    }

    /// True if the consumer has closed its read end.
    pub fn is_reader_closed(&self) -> bool {
        self.header().reader_closed.load(Ordering::Acquire)
    }

    /// True when both ends of this ring are closed.
    pub fn is_closed(&self) -> bool {
        self.header().writer_closed.load(Ordering::Acquire)
            && self.header().reader_closed.load(Ordering::Acquire)
    }
}

impl<R: SharedMemoryRegion> traits::Write for RingProducer<R> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError> {
        if buf.is_empty() {
            return Ok(0);
        }
        // Broken pipe: the reader has gone away, so no write can ever be
        // consumed. Surface this rather than silently buffering into a dead ring.
        if self.header().reader_closed.load(Ordering::Acquire) {
            return Err(PipeError::Closed);
        }
        let free = self.header().writable_len(self.data.size());
        if free == 0 {
            return Err(PipeError::WouldBlock);
        }

        let n = core::cmp::min(buf.len() as u64, free) as usize;
        let cursor = self.header().write_cursor.load(Ordering::Relaxed);
        self.data.write_at(cursor, &buf[..n]);

        // No standalone fence needed, release on the store guarantees the
        // preceding write_at is visible before the cursor update
        self.header()
            .write_cursor
            .store(cursor + n as u64, Ordering::Release);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::shared_memory_region::RawSharedMemoryRegion;
    use crate::pipe::traits::Write;
    use core::sync::atomic::AtomicU64;

    /// Region handle for tests. Only needed to satisfy ownership/liveness — the
    /// ring's actual header/data pointers are supplied separately below.
    fn raw(mem: &mut [u8]) -> RawSharedMemoryRegion {
        unsafe { RawSharedMemoryRegion::from_raw(mem.as_mut_ptr(), mem.len() as u64) }
    }

    fn header() -> RingHeader {
        use core::sync::atomic::AtomicBool;
        RingHeader {
            read_cursor: AtomicU64::new(0),
            write_cursor: AtomicU64::new(0),
            writer_closed: AtomicBool::new(false),
            reader_closed: AtomicBool::new(false),
        }
    }

    #[test]
    fn simple_write() {
        let h = header();
        let mut mem = [0u8; 16];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 16) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        assert_eq!(p.write(b"hello").unwrap(), 5);
        assert_eq!(&mem[..5], b"hello");
        assert_eq!(h.write_cursor.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn fill_to_full() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        assert_eq!(p.write(b"abcdefghij").unwrap(), 8);
        assert_eq!(&mem, b"abcdefgh");
    }

    #[test]
    fn wrap_around() {
        let h = header();
        h.read_cursor.store(5, Ordering::Relaxed);
        h.write_cursor.store(5, Ordering::Relaxed);
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        assert_eq!(p.write(b"XYZW").unwrap(), 4);
        assert_eq!(&mem[5..8], b"XYZ");
        assert_eq!(&mem[..1], b"W");
    }

    #[test]
    fn full_ring_blocks() {
        let h = header();
        h.write_cursor.store(4, Ordering::Relaxed);
        let mut mem = [0u8; 4];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        assert!(matches!(p.write(b"x"), Err(PipeError::WouldBlock)));
    }

    #[test]
    fn write_to_reader_closed_errs() {
        let h = header();
        h.reader_closed.store(true, Ordering::Release);
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        // Reader gone: broken pipe, not a transient WouldBlock.
        assert!(matches!(p.write(b"x"), Err(PipeError::Closed)));
    }

    #[test]
    fn zero_length_write_non_full() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        assert_eq!(p.write(b"").unwrap(), 0);
        assert_eq!(h.write_cursor.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn zero_length_write_full() {
        let h = header();
        h.write_cursor.store(8, Ordering::Relaxed);
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        assert_eq!(p.write(b"").unwrap(), 0);
    }

    #[test]
    fn bytes_pending_empty() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let p = RingProducer::new(raw(&mut mem), &h, data);
        assert_eq!(p.bytes_pending(), 0);
    }

    #[test]
    fn bytes_pending_after_write() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        p.write(b"hello").unwrap();
        assert_eq!(p.bytes_pending(), 5);
    }

    #[test]
    fn bytes_pending_zero_after_full_read() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(raw(&mut mem), &h, data);
        p.write(b"hello").unwrap();
        h.read_cursor.store(5, Ordering::Release);
        assert_eq!(p.bytes_pending(), 0);
    }
}
