use crate::pipe::error::PipeError;
use crate::pipe::ring::{RingData, RingHeader};
use crate::pipe::ring_consumer::RingConsumer;
use crate::pipe::ring_producer::RingProducer;
use crate::pipe::shared_memory_region::SharedMemoryRegion;
use crate::pipe::traits;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

/// One endpoint of a bidirectional pipe.
///
/// Memory layout: `[HeaderA][Ring A->B data][HeaderB][Ring B->A data]`.
pub struct BidirectionalPipe<'a, D: traits::DoorBell> {
    writer: RingProducer<'a>,
    reader: RingConsumer<'a>,
    read_available_doorbell: D,
    write_available_doorbell: D,
}

const HEADER_SIZE: u64 = core::mem::size_of::<RingHeader>() as u64;

impl<'a, D: traits::DoorBell> BidirectionalPipe<'a, D> {
    /// Total bytes of shared memory needed for a given `ring_size`.
    pub const fn required_size(ring_size: u64) -> u64 {
        2 * (HEADER_SIZE + ring_size)
    }

    /// Create a pipe endpoint over a shared memory region.
    ///
    /// Caller must ensure the region is zero-initialized before the first side
    /// is constructed, and that exactly one `Side::A` and one `Side::B` are
    /// created per region.
    pub fn new(
        region: &'a SharedMemoryRegion,
        ring_size: u64,
        side: Side,
        read_available_doorbell: D,
        write_available_doorbell: D,
    ) -> Self {
        assert!(region.len() >= Self::required_size(ring_size));
        assert!(
            ring_size.is_multiple_of(core::mem::align_of::<RingHeader>() as u64),
            "ring_size must be a multiple of 8 for header alignment"
        );
        let base = region.as_ptr();
        assert!(
            base.align_offset(core::mem::align_of::<RingHeader>()) == 0,
            "shared memory region must be 8-byte aligned"
        );

        // Layout: [HeaderA (16)] [DataA (ring_size)] [HeaderB (16)] [DataB (ring_size)]
        // Interleaved so each header is adjacent to its data (cache locality)
        // and headers are separated by ring_size (avoids false sharing).
        let header_a = unsafe { &*(base as *const RingHeader) };
        let data_a = unsafe { base.add(HEADER_SIZE as usize) };
        let header_b = unsafe { &*(data_a.add(ring_size as usize) as *const RingHeader) };
        let data_b = unsafe { data_a.add(ring_size as usize + HEADER_SIZE as usize) };

        let (writer_header, writer_data, reader_header, reader_data) = match side {
            Side::A => (header_a, data_a, header_b, data_b),
            Side::B => (header_b, data_b, header_a, data_a),
        };

        let writer = RingProducer::new(writer_header, unsafe {
            RingData::new(writer_data, ring_size)
        });
        let reader = RingConsumer::new(reader_header, unsafe {
            RingData::new(reader_data, ring_size)
        });
        Self {
            writer,
            reader,
            read_available_doorbell,
            write_available_doorbell,
        }
    }

    /// Split into independent read and write halves (like `TcpStream::split`).
    pub fn split(&mut self) -> (&mut RingConsumer<'a>, &mut RingProducer<'a>) {
        (&mut self.reader, &mut self.writer)
    }

    /// Close this side's outgoing (write) direction.
    pub fn close_write(&self) {
        self.writer.close_writer();
    }

    /// Close this side's incoming (read) direction.
    pub fn close_read(&self) {
        self.reader.close_reader();
    }

    /// True if the peer has closed their write side (no more data incoming).
    pub fn is_peer_write_closed(&self) -> bool {
        self.reader.is_writer_closed()
    }

    /// True if the peer has closed their read side (they will not read more data we send).
    pub fn is_peer_read_closed(&self) -> bool {
        self.writer.is_reader_closed()
    }

    /// True when both unidirectional rings are fully closed (all four flags set).
    pub fn is_closed(&self) -> bool {
        self.writer.is_closed() && self.reader.is_closed()
    }
}

impl<'a, D: traits::DoorBell> traits::Read for BidirectionalPipe<'a, D> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
        let res = self.reader.read(buf);
        if let Ok(s) = res {
            if s > 0 {
                self.write_available_doorbell.ring();
            }
        }
        res
    }
}

impl<'a, D: traits::DoorBell> traits::Write for BidirectionalPipe<'a, D> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError> {
        let res = self.writer.write(buf);
        if let Ok(s) = res {
            if s > 0 {
                self.read_available_doorbell.ring();
            }
        }
        res
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::pipe::DoorBell;
    use core::sync::atomic::{AtomicUsize, Ordering};

    pub struct TestDoorBell {
        count: AtomicUsize,
    }

    impl DoorBell for TestDoorBell {
        fn ring(&self) {
            self.count.fetch_add(1, Ordering::Release);
        }
    }

    impl TestDoorBell {
        pub fn new() -> Self {
            Self {
                count: AtomicUsize::new(0),
            }
        }
    }

    #[macro_export]
    macro_rules! pipe_pair {
        ($ring:expr, $mem:ident, $a:ident, $b:ident) => {
            let mut $mem = Aligned(
                [0u8; BidirectionalPipe::<TestDoorBell>::required_size($ring as u64) as usize],
            );
            let region =
                unsafe { SharedMemoryRegion::from_raw($mem.0.as_mut_ptr(), $mem.0.len() as u64) };
            #[allow(unused_mut)]
            let mut $a = BidirectionalPipe::new(
                &region,
                $ring,
                Side::A,
                TestDoorBell::new(),
                TestDoorBell::new(),
            );
            #[allow(unused_mut)]
            let mut $b = BidirectionalPipe::new(
                &region,
                $ring,
                Side::B,
                TestDoorBell::new(),
                TestDoorBell::new(),
            );
        };
    }

    pub(crate) use pipe_pair;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::traits::{Read, Write};
    use test_utils::{pipe_pair, TestDoorBell};

    #[repr(align(8))]
    struct Aligned<const N: usize>([u8; N]);

    #[test]
    fn required_size_matches_layout() {
        assert_eq!(
            BidirectionalPipe::<TestDoorBell>::required_size(64),
            2 * (24 + 64)
        );
    }

    #[test]
    fn round_trip_a_to_b() {
        pipe_pair!(64, mem, a, b);
        assert_eq!(a.write(b"ping").unwrap(), 4);
        let mut out = [0u8; 4];
        assert_eq!(b.read(&mut out).unwrap(), 4);
        assert_eq!(&out, b"ping");
    }

    #[test]
    fn round_trip_b_to_a() {
        pipe_pair!(32, mem, a, b);
        assert_eq!(b.write(b"pong!!").unwrap(), 6);
        let mut out = [0u8; 6];
        assert_eq!(a.read(&mut out).unwrap(), 6);
        assert_eq!(&out, b"pong!!");
    }

    #[test]
    fn both_directions_independent() {
        pipe_pair!(32, mem, a, b);
        a.write(b"hello").unwrap();
        b.write(b"world").unwrap();

        let mut from_a = [0u8; 5];
        let mut from_b = [0u8; 5];
        b.read(&mut from_a).unwrap();
        a.read(&mut from_b).unwrap();
        assert_eq!(&from_a, b"hello");
        assert_eq!(&from_b, b"world");
    }

    #[test]
    fn multi_lap() {
        pipe_pair!(8, mem, a, b);
        for i in 0u8..20 {
            assert_eq!(a.write(&[i]).unwrap(), 1);
            let mut out = [0u8; 1];
            assert_eq!(b.read(&mut out).unwrap(), 1);
            assert_eq!(out[0], i);
        }
    }

    #[test]
    fn fill_drain_refill() {
        pipe_pair!(8, mem, a, b);
        assert_eq!(a.write(b"12345678").unwrap(), 8);
        let mut out = [0u8; 8];
        assert_eq!(b.read(&mut out).unwrap(), 8);
        assert_eq!(&out, b"12345678");

        assert_eq!(a.write(b"abcdefgh").unwrap(), 8);
        assert_eq!(b.read(&mut out).unwrap(), 8);
        assert_eq!(&out, b"abcdefgh");
    }

    #[test]
    fn interleaved_both_directions() {
        pipe_pair!(16, mem, a, b);
        a.write(b"aa").unwrap();
        b.write(b"bb").unwrap();
        a.write(b"cc").unwrap();
        b.write(b"dd").unwrap();

        let mut out = [0u8; 4];
        b.read(&mut out).unwrap();
        assert_eq!(&out, b"aacc");
        a.read(&mut out).unwrap();
        assert_eq!(&out, b"bbdd");
    }
}
