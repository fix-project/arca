use crate::pipe::{BidirectionalPipe, DoorBell, PipeError, Read, Write};

#[derive(Debug)]
pub enum StreamError {
    WriteClosed,
}

pub struct SyncStream<'a, D: DoorBell> {
    /// BuddyAllocator offset of the SHM region backing this pipe.
    pub shm_offset: u64,
    pipe: BidirectionalPipe<'a, D>,
}

impl<'a, D: DoorBell> SyncStream<'a, D> {
    pub fn from_pipe(shm_offset: u64, pipe: BidirectionalPipe<'a, D>) -> Self {
        Self { shm_offset, pipe }
    }

    /// Write all of `buf` into the pipe, spinning if the ring is full; returns `Err(WriteClosed)` if the peer closed their read side.
    pub fn send(&mut self, buf: &[u8]) -> Result<usize, StreamError> {
        if self.pipe.is_peer_read_closed() {
            self.pipe.close_write();
            return Err(StreamError::WriteClosed);
        }
        if buf.is_empty() {
            return Ok(0);
        }
        self.pipe.write_all(buf);
        Ok(buf.len())
    }

    /// Read exactly `buf.len()` bytes, spinning until full; returns `Ok(n < buf.len())` only on EOF when the peer closed their write side.
    pub fn recv(&mut self, buf: &mut [u8]) -> Result<usize, StreamError> {
        let n = read_exact(&mut self.pipe, buf);
        if n < buf.len() {
            self.pipe.close_read();
        }
        Ok(n)
    }

    pub fn close_write(&mut self) {
        self.pipe.close_write();
    }

    pub fn close_read(&mut self) {
        self.pipe.close_read();
    }

    pub fn is_closed(&self) -> bool {
        self.pipe.is_closed()
    }
}

fn read_exact<D: DoorBell>(pipe: &mut crate::pipe::BidirectionalPipe<D>, buf: &mut [u8]) -> usize {
    let mut filled = 0;
    while filled < buf.len() {
        match pipe.read(&mut buf[filled..]) {
            Ok(n) => filled += n,
            Err(PipeError::WouldBlock) => {
                if pipe.is_peer_write_closed() {
                    break;
                }
            }
        }
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::test_utils::pipe_pair;
    use crate::pipe::test_utils::TestDoorBell;
    use crate::pipe::{BidirectionalPipe, SharedMemoryRegion, Side};

    #[repr(align(8))]
    struct Aligned<const N: usize>([u8; N]);

    macro_rules! stream_pair {
        ($ring:expr, $mem:ident, $a:ident, $b:ident) => {
            pipe_pair!($ring, $mem, pipe_a, pipe_b);
            let mut $a = SyncStream::from_pipe(0, pipe_a);
            let mut $b = SyncStream::from_pipe(0, pipe_b);
        };
    }

    #[test]
    fn send_recv_data() {
        stream_pair!(128, mem, a, b);
        assert_eq!(a.send(b"hello").unwrap(), 5);
        let mut buf = [0u8; 5];
        assert_eq!(b.recv(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"hello");
    }

    #[test]
    fn close_write_signals_eof_to_peer() {
        stream_pair!(64, mem, a, b);
        a.close_write();
        let mut buf = [0u8; 8];
        assert_eq!(b.recv(&mut buf).unwrap(), 0);
        assert!(!b.is_closed());
    }

    #[test]
    fn close_both_sides_blocks_peer_ops() {
        stream_pair!(64, mem, a, b);
        b.close_write();
        b.close_read();
        // b has closed its own ends but a hasn't yet — pipe not fully closed
        assert!(!b.is_closed());
        let mut buf = [0u8; 8];
        // a sees EOF because b closed write, and WriteClosed because b closed read
        assert_eq!(a.recv(&mut buf).unwrap(), 0);
        assert!(matches!(a.send(b"x"), Err(StreamError::WriteClosed)));
    }

    #[test]
    fn send_after_peer_closes_read_errors() {
        stream_pair!(64, mem, a, b);
        b.close_read();
        assert!(matches!(a.send(b"x"), Err(StreamError::WriteClosed)));
    }

    #[test]
    fn recv_after_eof_returns_zero() {
        stream_pair!(64, mem, a, b);
        a.close_write();
        let mut buf = [0u8; 8];
        b.recv(&mut buf).unwrap();
        assert_eq!(b.recv(&mut buf).unwrap(), 0);
    }

    #[test]
    fn recv_fills_exact_buffer_size() {
        stream_pair!(128, mem, a, b);
        assert_eq!(a.send(b"hello").unwrap(), 5);
        let mut buf = [0u8; 5];
        assert_eq!(b.recv(&mut buf).unwrap(), 5);
        assert_eq!(&buf, b"hello");
    }

    #[test]
    fn pipe_closed_after_both_sides_close() {
        stream_pair!(128, mem, a, b);
        a.close_write();
        let mut buf = [0u8; 8];
        b.recv(&mut buf).unwrap();
        b.close_write();
        a.recv(&mut buf).unwrap();
        assert!(a.is_closed());
    }
}
