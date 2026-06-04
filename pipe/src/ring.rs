use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Header for a single SPSC ring buffer, stored in shared memory.
///
/// Cursors are monotonically increasing logical offsets. Physical positions
/// are `cursor % ring_size`. The close flags signal orderly shutdown: the
/// producer sets `writer_closed`; the consumer sets `reader_closed`.
#[repr(C)]
pub struct RingHeader {
    pub read_cursor: AtomicU64,
    pub write_cursor: AtomicU64,
    pub writer_closed: AtomicBool,
    pub reader_closed: AtomicBool,
}

impl RingHeader {
    /// Bytes available to read. Called by the consumer.
    pub fn used_space(&self) -> u64 {
        let write = self.write_cursor.load(Ordering::Acquire);
        let read = self.read_cursor.load(Ordering::Relaxed);

        // Cursors are monotonically increasing and never reset, but they can
        // overflow u64. wrapping_sub gives the correct delta regardless, 
        // also avoids panic on debug-mode subtraction overflow
        write.wrapping_sub(read)
    }

    /// Bytes available to write. Called by the producer.
    pub fn free_space(&self, capacity: u64) -> u64 {
        let write = self.write_cursor.load(Ordering::Relaxed);
        let read = self.read_cursor.load(Ordering::Acquire);

        // See used_space — wrapping_sub handles cursor overflow correctly
        capacity - write.wrapping_sub(read)
    }
}

/// Raw data region of a single SPSC ring buffer.
///
/// Owns `(ptr, size)` together so call sites don't juggle them.
/// Wrap-around is handled inside `write_at` / `read_at`.
pub struct RingData {
    ptr: *mut u8,
    size: u64,
}

impl RingData {
    /// # Safety
    /// - `ptr` must point to a valid region of `size` bytes.
    /// - Caller must guarantee SPSC discipline on top of this region.
    pub unsafe fn new(ptr: *mut u8, size: u64) -> Self {
        Self { ptr, size }
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    /// Write `buf` starting at physical offset `cursor % size`, wrapping if needed.
    /// Caller must ensure `buf.len() <= free space`.
    pub fn write_at(&mut self, cursor: u64, buf: &[u8]) {
        let size = self.size as usize;
        let offset = (cursor % self.size) as usize;
        let first = core::cmp::min(buf.len(), size - offset);
        unsafe {
            core::ptr::copy_nonoverlapping(buf.as_ptr(), self.ptr.add(offset), first);
            if buf.len() > first {
                core::ptr::copy_nonoverlapping(
                    buf.as_ptr().add(first),
                    self.ptr,
                    buf.len() - first,
                );
            }
        }
    }

    /// Read into `buf` starting at physical offset `cursor % size`, wrapping if needed.
    /// Caller must ensure `buf.len() <= used space`.
    pub fn read_at(&self, cursor: u64, buf: &mut [u8]) {
        let size = self.size as usize;
        let offset = (cursor % self.size) as usize;
        let first = core::cmp::min(buf.len(), size - offset);
        unsafe {
            core::ptr::copy_nonoverlapping(self.ptr.add(offset), buf.as_mut_ptr(), first);
            if buf.len() > first {
                core::ptr::copy_nonoverlapping(
                    self.ptr,
                    buf.as_mut_ptr().add(first),
                    buf.len() - first,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_with(read: u64, write: u64) -> RingHeader {
        RingHeader {
            read_cursor: AtomicU64::new(read),
            write_cursor: AtomicU64::new(write),
            writer_closed: AtomicBool::new(false),
            reader_closed: AtomicBool::new(false),
        }
    }

    #[test]
    fn empty_ring() {
        let h = header_with(0, 0);
        assert_eq!(h.used_space(), 0);
        assert_eq!(h.free_space(64), 64);
    }

    #[test]
    fn partial_fill() {
        let h = header_with(10, 40);
        assert_eq!(h.used_space(), 30);
        assert_eq!(h.free_space(64), 34);
    }

    #[test]
    fn full_ring() {
        let h = header_with(100, 164);
        assert_eq!(h.used_space(), 64);
        assert_eq!(h.free_space(64), 0);
    }

    #[test]
    fn data_write_then_read_no_wrap() {
        let mut mem = [0u8; 8];
        let mut rd = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        rd.write_at(0, b"abcd");
        let mut out = [0u8; 4];
        rd.read_at(0, &mut out);
        assert_eq!(&out, b"abcd");
    }

    #[test]
    fn data_write_wraps() {
        let mut mem = [0u8; 8];
        let mut rd = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        rd.write_at(6, b"XYZW");
        assert_eq!(&mem[6..8], b"XY");
        assert_eq!(&mem[..2], b"ZW");
    }

    #[test]
    fn data_read_wraps() {
        let mut mem = *b"cdEFabXY";
        let rd = unsafe { RingData::new(mem.as_mut_ptr(), 8) };
        let mut out = [0u8; 6];
        rd.read_at(6, &mut out);
        assert_eq!(&out, b"XYcdEF");
    }
}
