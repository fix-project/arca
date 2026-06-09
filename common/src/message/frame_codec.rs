extern crate alloc;

use crate::pipe::{PipeError, Read, Write};
use alloc::boxed::Box;

/// Errors from moving frames over a transport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Declared payload length exceeds [`MAX_FRAME_PAYLOAD`].
    PayloadTooLarge {
        len: usize,
    },
    /// Transport returned `Ok(0)` — the peer hung up.
    Closed,
    UnfinishedPayload,
    NoPayload,
}

/// One frame in memory (inline payload buffer).
///
/// The `payload` array is fixed-size so `ControlFrame` is `Copy` and lives
/// happily in `no_std`. Only the first `payload_len` bytes are valid; use
/// [`ControlFrame::payload`] to get the live slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<const MAX_FRAME_PAYLOAD: usize> {
    pub payload_len: usize,
    pub payload: [u8; MAX_FRAME_PAYLOAD],
}

impl<const MAX_FRAME_PAYLOAD: usize> Frame<MAX_FRAME_PAYLOAD> {
    pub fn as_slice(&self) -> &[u8] {
        &self.payload[..self.payload_len]
    }
}

/// Incremental frame reader for non-blocking transports.
///
/// Keeps partial bytes until a full frame is available; supports multiple
/// frames read in one chunk.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct FrameReadBuf<const MAX_FRAME_PAYLOAD: usize> {
    header_storage: [u8; 2],
    payload_storage: [u8; MAX_FRAME_PAYLOAD],
    header_len: usize,
    len: usize,
}

impl<const MAX_FRAME_PAYLOAD: usize> Default for FrameReadBuf<MAX_FRAME_PAYLOAD> {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(unused)]
impl<const MAX_FRAME_PAYLOAD: usize> FrameReadBuf<MAX_FRAME_PAYLOAD> {
    pub const fn new() -> Self {
        Self {
            header_storage: [0u8; 2],
            payload_storage: [0u8; MAX_FRAME_PAYLOAD],
            header_len: 0,
            len: 0,
        }
    }

    /// Append bytes from `transport` and return the next full frame, if any.
    ///
    /// Returns `Ok(None)` when starved by `WouldBlock` or a partial frame.
    pub fn try_read_frame<T: Read>(
        &mut self,
        transport: &mut T,
    ) -> Result<Option<Frame<MAX_FRAME_PAYLOAD>>, Error> {
        loop {
            if self.header_len < 2 {
                match transport.read(&mut self.header_storage[self.header_len..]) {
                    Ok(0) => return Err(Error::Closed),
                    Ok(n) => self.header_len += n,
                    Err(PipeError::WouldBlock) => return Ok(None),
                }
            } else {
                let payload_len = u16::from_le_bytes(self.header_storage) as usize;
                if payload_len > MAX_FRAME_PAYLOAD {
                    return Err(Error::PayloadTooLarge { len: payload_len });
                }

                match transport.read(&mut self.payload_storage[self.len..payload_len]) {
                    Ok(0) => return Err(Error::Closed),
                    Ok(n) => self.len += n,
                    Err(PipeError::WouldBlock) => return Ok(None),
                }

                if self.len == payload_len {
                    let frame = Frame::<MAX_FRAME_PAYLOAD> {
                        payload: self.payload_storage,
                        payload_len,
                    };

                    // Reset state
                    self.header_storage = [0u8; 2];
                    self.payload_storage = [0u8; MAX_FRAME_PAYLOAD];
                    self.header_len = 0;
                    self.len = 0;

                    return Ok(Some(frame));
                }
            }
        }
    }
}

/// Incremental frame writer for non-blocking transports.
#[derive(Debug, Clone)]
#[allow(unused)]
pub struct FrameWriteBuf {
    written_len: usize,
    frame: Option<Box<[u8]>>,
}

impl Default for FrameWriteBuf {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(unused)]
impl FrameWriteBuf {
    pub const fn new() -> Self {
        Self {
            written_len: 0,
            frame: None,
        }
    }

    pub fn load(&mut self, frame: Box<[u8]>) -> Result<(), Error> {
        if self.frame.is_some() {
            Err(Error::UnfinishedPayload)
        } else {
            self.frame = Some(frame);
            Ok(())
        }
    }

    /// Append bytes from `buf` to `transport` and return the next full frame, if any.
    ///
    /// Returns `Ok(false)` when starved by `WouldBlock`.
    /// Returns `Ok(true)` when finished writing a frame.`
    pub fn try_write_frame<T: Write>(&mut self, transport: &mut T) -> Result<bool, Error> {
        loop {
            if self.frame.is_none() {
                return Err(Error::NoPayload);
            }

            if self.written_len < 2 {
                let payload_len =
                    (u16::try_from(self.frame.as_ref().unwrap().len()).unwrap()).to_le_bytes();

                match transport.write(&payload_len[self.written_len..]) {
                    Ok(0) => return Err(Error::Closed),
                    Ok(n) => self.written_len += n,
                    Err(PipeError::WouldBlock) => return Ok(false),
                }
            } else {
                match transport.write(&self.frame.as_ref().unwrap()[self.written_len - 2..]) {
                    Ok(0) => return Err(Error::Closed),
                    Ok(n) => self.written_len += n,
                    Err(PipeError::WouldBlock) => return Ok(false),
                }

                if self.written_len == self.frame.as_ref().unwrap().len() + 2 {
                    self.written_len = 0;
                    self.frame = None;
                    return Ok(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MemPipe<const N: usize> {
        buf: [u8; N],
        len: usize,
        pos: usize,
    }

    impl<const N: usize> MemPipe<N> {
        fn new() -> Self {
            Self {
                buf: [0u8; N],
                len: 0,
                pos: 0,
            }
        }
    }

    impl<const N: usize> Write for MemPipe<N> {
        fn write(&mut self, src: &[u8]) -> Result<usize, PipeError> {
            let n = src.len();
            assert!(self.len + n <= N, "MemPipe full");
            self.buf[self.len..self.len + n].copy_from_slice(src);
            self.len += n;
            Ok(n)
        }
    }

    impl<const N: usize> Read for MemPipe<N> {
        fn read(&mut self, dst: &mut [u8]) -> Result<usize, PipeError> {
            if self.pos >= self.len {
                return Err(PipeError::WouldBlock);
            }
            let n = core::cmp::min(dst.len(), self.len - self.pos);
            dst[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn write_read_round_trip() {
        let mut pipe = MemPipe::<256>::new();
        let frame = vec![1u8, 2u8, 3u8, 4u8].into_boxed_slice();

        let mut reader = FrameReadBuf::<256>::new();
        let mut writer = FrameWriteBuf::new();

        writer.load(frame).unwrap();
        let res = writer.try_write_frame(&mut pipe).unwrap();
        assert!(res);

        let got = reader.try_read_frame(&mut pipe).unwrap().unwrap();
        assert_eq!(got.payload_len, 4);
        assert_eq!(got.as_slice(), [1u8, 2u8, 3u8, 4u8]);
    }

    #[test]
    fn write_read_two_frames_in_order() {
        let mut pipe = MemPipe::<1024>::new();

        let frame1 = vec![1u8, 2u8, 3u8, 4u8].into_boxed_slice();
        let frame2 = vec![5u8, 6u8, 7u8, 8u8].into_boxed_slice();

        let mut writer = FrameWriteBuf::new();
        let mut reader = FrameReadBuf::<256>::new();

        writer.load(frame1).unwrap();
        let res = writer.try_write_frame(&mut pipe).unwrap();
        assert!(res);
        writer.load(frame2).unwrap();
        let res = writer.try_write_frame(&mut pipe).unwrap();
        assert!(res);

        let got_a = reader.try_read_frame(&mut pipe).unwrap().unwrap();
        assert_eq!(got_a.as_slice(), [1u8, 2u8, 3u8, 4u8]);
        let got_b = reader.try_read_frame(&mut pipe).unwrap().unwrap();
        assert_eq!(got_b.as_slice(), [5u8, 6u8, 7u8, 8u8]);
    }

    #[test]
    fn frame_read_buf_one_byte_at_a_time() {
        let pay = 3u32.to_le_bytes();
        let pay: Box<[u8]> = Box::new(pay);
        let mut wire = MemPipe::<256>::new();

        let mut writer = FrameWriteBuf::new();
        writer.load(pay).unwrap();
        let res = writer.try_write_frame(&mut wire).unwrap();
        assert!(res);

        struct OneByte {
            pipe: MemPipe<256>,
        }

        impl Read for OneByte {
            fn read(&mut self, dst: &mut [u8]) -> Result<usize, PipeError> {
                let mut buf = [0u8; 1];
                self.pipe.read(&mut buf)?;
                dst[0] = buf[0];
                Ok(1)
            }
        }

        let mut reader = OneByte { pipe: wire };

        let mut dec = FrameReadBuf::<256>::new();
        let mut decoded = None;
        for _ in 0..4096 {
            if let Some(f) = dec.try_read_frame(&mut reader).unwrap() {
                decoded = Some(f);
                break;
            }
        }
        let got = decoded.expect("should decode after enough bytes");
        assert_eq!(got.as_slice(), 3u32.to_le_bytes());
    }
}
