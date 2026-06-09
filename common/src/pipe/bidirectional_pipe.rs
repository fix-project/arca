use crate::pipe::error::PipeError;
use crate::pipe::ring::{RingData, RingHeader};
use crate::pipe::ring_consumer::RingConsumer;
use crate::pipe::ring_producer::RingProducer;
use crate::pipe::shared_memory_region::SharedMemoryRegion;
use crate::pipe::traits::{self, DoorBell, OwnedSplit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

/// One endpoint of a bidirectional pipe.
///
/// Owns a [`RingProducer`] and [`RingConsumer`], each of which holds its own
/// clone of the [`SharedMemoryRegion`] handle — so the pipe carries no lifetime
/// and is not tied to a borrowed region. [`split`](Self::split) hands those two
/// ends back as fully owned, `Send` values.
///
/// The pipe (and the ends returned by [`split`](Self::split)) are `Send`
/// whenever `R: Send`.
///
/// Memory layout: `[HeaderA][Ring A->B data][HeaderB][Ring B->A data]`.
pub struct BidirectionalPipe<R: SharedMemoryRegion, D: DoorBell> {
    writer: RingProducer<R>,
    reader: RingConsumer<R>,
    read_available_doorbell: D,
    write_available_doorbell: D,
}

#[allow(unused)]
pub struct BidirectionalPipeWriteEnd<R: SharedMemoryRegion, D: DoorBell> {
    writer: RingProducer<R>,
    read_available_doorbell: D,
}

#[allow(unused)]
pub struct BidirectionalPipeReadEnd<R: SharedMemoryRegion, D: DoorBell> {
    reader: RingConsumer<R>,
    write_available_doorbell: D,
}

const HEADER_SIZE: u64 = core::mem::size_of::<RingHeader>() as u64;

impl<R: SharedMemoryRegion, D: DoorBell> BidirectionalPipe<R, D> {
    /// Total bytes of shared memory needed for a given `ring_size`.
    pub const fn required_size(ring_size: u64) -> u64 {
        2 * (HEADER_SIZE + ring_size)
    }

    /// Create a pipe endpoint that takes ownership of a shared memory region.
    ///
    /// The region handle is cloned into each ring end so the reader and writer
    /// each keep the mapping alive on their own. Caller must ensure the region
    /// is zero-initialized before the first side is constructed, and that
    /// exactly one `Side::A` and one `Side::B` are created per region.
    pub fn new(
        region: R,
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

        // Layout: [HeaderA (24)] [DataA (ring_size)] [HeaderB (24)] [DataB (ring_size)]
        // Interleaved so each header is adjacent to its data (cache locality)
        // and headers are separated by ring_size (avoids false sharing).
        let header_a = base as *const RingHeader;
        let data_a = unsafe { base.add(HEADER_SIZE as usize) };
        let header_b = unsafe { data_a.add(ring_size as usize) as *const RingHeader };
        let data_b = unsafe { data_a.add(ring_size as usize + HEADER_SIZE as usize) };

        let (writer_header, writer_data, reader_header, reader_data) = match side {
            Side::A => (header_a, data_a, header_b, data_b),
            Side::B => (header_b, data_b, header_a, data_a),
        };

        let writer = RingProducer::new(region.clone(), writer_header, unsafe {
            RingData::new(writer_data, ring_size)
        });
        let reader = RingConsumer::new(region, reader_header, unsafe {
            RingData::new(reader_data, ring_size)
        });
        Self {
            writer,
            reader,
            read_available_doorbell,
            write_available_doorbell,
        }
    }

    /// Consume the pipe and split it into independent, owned read and write
    /// ends (like `TcpStream::into_split`).
    ///
    /// Each end already owns its own clone of the region handle, so the two can
    /// be moved to separate threads / async tasks and each independently keeps
    /// the shared mapping alive.
    pub fn split(
        self,
    ) -> (
        BidirectionalPipeReadEnd<R, D>,
        BidirectionalPipeWriteEnd<R, D>,
    ) {
        (
            BidirectionalPipeReadEnd {
                reader: self.reader,
                write_available_doorbell: self.write_available_doorbell,
            },
            BidirectionalPipeWriteEnd {
                writer: self.writer,
                read_available_doorbell: self.read_available_doorbell,
            },
        )
    }

    /// Close this side's outgoing (write) direction.
    pub fn close_write(&mut self) {
        self.writer.close_writer();
    }

    /// Close this side's incoming (read) direction.
    pub fn close_read(&mut self) {
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

impl<R: SharedMemoryRegion, D: DoorBell> traits::Read for BidirectionalPipe<R, D> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
        let res = self.reader.read(buf);
        if let Ok(s) = res {
            self.write_available_doorbell.ring();
            if s > 0 {}
        }
        res
    }
}

impl<R: SharedMemoryRegion, D: DoorBell> traits::Read for BidirectionalPipeReadEnd<R, D> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, PipeError> {
        let res = self.reader.read(buf);
        if let Ok(s) = res {
            self.write_available_doorbell.ring();
            if s > 0 {}
        }
        res
    }
}

impl<R: SharedMemoryRegion, D: traits::DoorBell> traits::Write for BidirectionalPipe<R, D> {
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

impl<R: SharedMemoryRegion, D: DoorBell> traits::Write for BidirectionalPipeWriteEnd<R, D> {
    fn write(&mut self, buf: &[u8]) -> Result<usize, PipeError> {
        self.writer.write(buf)
    }
}

impl<R: SharedMemoryRegion + Send + 'static, D: DoorBell + Send + 'static> OwnedSplit
    for BidirectionalPipe<R, D>
{
    fn split(
        self,
    ) -> (
        impl traits::Read + Send + 'static,
        impl traits::Write + Send + 'static,
    ) {
        self.split()
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
                [0u8; BidirectionalPipe::<RawSharedMemoryRegion, TestDoorBell>::required_size(
                    $ring as u64,
                ) as usize],
            );
            let region = unsafe {
                RawSharedMemoryRegion::from_raw($mem.0.as_mut_ptr(), $mem.0.len() as u64)
            };
            #[allow(unused_mut)]
            let mut $a = BidirectionalPipe::new(
                region,
                $ring,
                Side::A,
                TestDoorBell::new(),
                TestDoorBell::new(),
            );
            #[allow(unused_mut)]
            let mut $b = BidirectionalPipe::new(
                region,
                $ring,
                Side::B,
                TestDoorBell::new(),
                TestDoorBell::new(),
            );
        };
    }

    pub(crate) use pipe_pair;
}

pub const HOST_SIDE: Side = Side::A;
pub const ARCA_SIDE: Side = Side::B;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::traits::{Read, Write};
    use crate::pipe::{RawSharedMemoryRegion, RingConsumer, RingProducer};
    use test_utils::{pipe_pair, TestDoorBell};

    #[repr(align(8))]
    struct Aligned<const N: usize>([u8; N]);

    #[test]
    fn required_size_matches_layout() {
        assert_eq!(
            BidirectionalPipe::<RawSharedMemoryRegion, TestDoorBell>::required_size(64),
            2 * (24 + 64)
        );
    }

    #[test]
    fn owned_split_round_trip() {
        pipe_pair!(32, mem, a, b);
        let (mut a_rx, mut a_tx) = a.split();
        let (mut b_rx, mut b_tx) = b.split();

        a_tx.write(b"hi").unwrap();
        let mut out = [0u8; 2];
        b_rx.read(&mut out).unwrap();
        assert_eq!(&out, b"hi");

        b_tx.write(b"yo").unwrap();
        a_rx.read(&mut out).unwrap();
        assert_eq!(&out, b"yo");
    }

    #[test]
    fn split_halves_and_pipe_are_send() {
        // The whole point of dropping the lifetime: owned, `Send` endpoints
        // that can move to other threads / async tasks.
        fn assert_send<T: Send>() {}
        assert_send::<BidirectionalPipe<RawSharedMemoryRegion, TestDoorBell>>();
        assert_send::<RingConsumer<RawSharedMemoryRegion>>();
        assert_send::<RingProducer<RawSharedMemoryRegion>>();
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

    #[test]
    fn read_sees_eof_after_peer_closes_write() {
        pipe_pair!(16, mem, a, b);
        a.write(b"bye").unwrap();
        a.close_write();
        let mut out = [0u8; 8];
        // Buffered bytes drain first...
        assert_eq!(b.read(&mut out).unwrap(), 3);
        assert_eq!(&out[..3], b"bye");
        // ...then EOF as Ok(0).
        assert_eq!(b.read(&mut out).unwrap(), 0);
    }

    #[test]
    fn write_errs_after_peer_closes_read() {
        pipe_pair!(16, mem, a, b);
        // B abandons the A->B direction; A's writes can never be consumed.
        b.close_read();
        assert!(matches!(a.write(b"x"), Err(PipeError::Closed)));
    }
}
