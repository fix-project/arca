use crate::error::PipeError;
use crate::ring::{RingData, RingHeader};
use crate::traits;
use core::sync::atomic::Ordering;

/// Consumer (read) end of a single SPSC ring buffer.
pub struct RingConsumer<'a> {
    header: &'a RingHeader,
    data: RingData,
}

impl<'a> RingConsumer<'a> {
    pub fn new(header: &'a RingHeader, data: RingData) -> Self {
        Self { header, data }
    }

    /// Signal that this consumer will read no more bytes.
    pub fn close_reader(&self) {
        self.header.reader_closed.store(true, Ordering::Release);
    }

    /// True if the producer has closed its write end.
    pub fn is_writer_closed(&self) -> bool {
        self.header.writer_closed.load(Ordering::Acquire)
    }

    /// True when both ends of this ring are closed.
    pub fn is_closed(&self) -> bool {
        self.header.writer_closed.load(Ordering::Acquire)
            && self.header.reader_closed.load(Ordering::Acquire)
    }
}

impl<'a> traits::Read for RingConsumer<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
        if buf.is_empty() { return Ok(0); }
        let used = self.header.used_space();
        if used == 0 {
            return Err(PipeError::WouldBlock);
        }

        let n = core::cmp::min(buf.len() as u64, used) as usize;
        let cursor = self.header.read_cursor.load(Ordering::Relaxed);
        self.data.read_at(cursor, &mut buf[..n]);

        // No standalone fence needed, release on the store guarantees the
        // preceding read_at is visible before the cursor update
        self.header.read_cursor.store(cursor + n as u64, Ordering::Release);
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::Read;
    use core::sync::atomic::AtomicU64;

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
        let mut c = RingConsumer::new(&h, data);
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
        let mut c = RingConsumer::new(&h, data);
        let mut out = [0u8; 3];
        assert_eq!(c.read(&mut out).unwrap(), 3);
        assert_eq!(&out, b"abc");
    }

    #[test]
    fn wrap_around() {
        let mut mem = *b"WXYZabcd";
        let h = header(5, 12);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut c = RingConsumer::new(&h, data);
        let mut out = [0u8; 8];
        assert_eq!(c.read(&mut out).unwrap(), 7);
        assert_eq!(&out[..7], b"bcdWXYZ");
    }

    #[test]
    fn empty_ring_blocks() {
        let mut mem = [0u8; 4];
        let h = header(4, 4);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(&h, data);
        let mut out = [0u8; 4];
        assert!(matches!(c.read(&mut out), Err(PipeError::WouldBlock)));
    }

    #[test]
    fn zero_length_read_non_empty() {
        let mut mem = *b"data";
        let h = header(0, 4);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(&h, data);
        let mut out = [0u8; 0];
        assert_eq!(c.read(&mut out).unwrap(), 0);
        assert_eq!(h.read_cursor.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn zero_length_read_empty() {
        let mut mem = [0u8; 4];
        let h = header(0, 0);
        let data = unsafe { RingData::new(mem.as_mut_ptr(), 4) };
        let mut c = RingConsumer::new(&h, data);
        let mut out = [0u8; 0];
        assert_eq!(c.read(&mut out).unwrap(), 0);
    }


}
