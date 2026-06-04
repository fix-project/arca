use crate::error::PipeError;
use crate::ring::{RingData, RingHeader};
use crate::traits;
use core::sync::atomic::Ordering;

/// Producer (write) end of a single SPSC ring buffer.
pub struct RingProducer<'a> {
    header: &'a RingHeader,
    data: RingData,
}

impl<'a> RingProducer<'a> {
    pub fn new(header: &'a RingHeader, data: RingData) -> Self {
        Self { header, data }
    }

    /// Bytes written by this producer that the consumer has not yet read.
    /// Uses Acquire on read_cursor so this can be called cross-thread safely.
    pub fn bytes_pending(&self) -> u64 {
        let write = self.header.write_cursor.load(Ordering::Relaxed);
        let read  = self.header.read_cursor.load(Ordering::Acquire);
        write.wrapping_sub(read)
    }

    /// Signal that this producer will write no more bytes.
    pub fn close_writer(&self) {
        self.header.writer_closed.store(true, Ordering::Release);
    }

    /// True if the consumer has closed its read end.
    pub fn is_reader_closed(&self) -> bool {
        self.header.reader_closed.load(Ordering::Acquire)
    }

    /// True when both ends of this ring are closed.
    pub fn is_closed(&self) -> bool {
        self.header.writer_closed.load(Ordering::Acquire)
            && self.header.reader_closed.load(Ordering::Acquire)
    }
}

impl<'a> traits::Write for RingProducer<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError> {
        if buf.is_empty() { return Ok(0); }
        let free = self.header.free_space(self.data.size());
        if free == 0 {
            return Err(PipeError::WouldBlock);
        }

        let n = core::cmp::min(buf.len() as u64, free) as usize;
        let cursor = self.header.write_cursor.load(Ordering::Relaxed);
        self.data.write_at(cursor, &buf[..n]);

        // No standalone fence needed, release on the store guarantees the
        // preceding write_at is visible before the cursor update
        self.header.write_cursor.store(cursor + n as u64, Ordering::Release);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Write;
    use core::sync::atomic::AtomicU64;

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
        let mut p = RingProducer::new(&h, data);
        assert_eq!(p.write(b"hello").unwrap(), 5);
        assert_eq!(&mem[..5], b"hello");
        assert_eq!(h.write_cursor.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn fill_to_full() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(&h, data);
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
        let mut p = RingProducer::new(&h, data);
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
        let mut p = RingProducer::new(&h, data);
        assert!(matches!(p.write(b"x"), Err(PipeError::WouldBlock)));
    }

    #[test]
    fn zero_length_write_non_full() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(&h, data);
        assert_eq!(p.write(b"").unwrap(), 0);
        assert_eq!(h.write_cursor.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn zero_length_write_full() {
        let h = header();
        h.write_cursor.store(8, Ordering::Relaxed);
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(&h, data);
        assert_eq!(p.write(b"").unwrap(), 0);
    }

    #[test]
    fn bytes_pending_empty() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let p = RingProducer::new(&h, data);
        assert_eq!(p.bytes_pending(), 0);
    }

    #[test]
    fn bytes_pending_after_write() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(&h, data);
        p.write(b"hello").unwrap();
        assert_eq!(p.bytes_pending(), 5);
    }

    #[test]
    fn bytes_pending_zero_after_full_read() {
        let h = header();
        let mut mem = [0u8; 8];
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut p = RingProducer::new(&h, data);
        p.write(b"hello").unwrap();
        h.read_cursor.store(5, Ordering::Release);
        assert_eq!(p.bytes_pending(), 0);
    }
}
