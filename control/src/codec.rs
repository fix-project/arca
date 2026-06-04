//! Read and write [`ControlFrame`] values on an `arca-pipe` byte stream.
//!
//! Header layout matches [`crate::protocol`].

use arca_pipe::{PipeError, Read, Write};

use crate::{CodecError, ControlFrame, MessageType, MAX_FRAME_PAYLOAD};

/// `message_type` (1) + `payload_len` (2) + `request_id` (4).
pub const HEADER_LEN: usize = 7;

/// Largest on-wire frame: header + full payload.
pub const MAX_WIRE_FRAME_LEN: usize = HEADER_LEN + MAX_FRAME_PAYLOAD;

#[inline]
fn relax_wait() {
    // Avoid busy-spinning on WouldBlock when multiple threads / the monitor
    // is waiting on the control ring.
    core::hint::spin_loop();
}

pub fn write_frame<T: Write>(transport: &mut T, frame: &ControlFrame) -> Result<(), CodecError> {
    let mut header = [0u8; HEADER_LEN];
    header[0] = frame.message_type as u8;
    header[1..3].copy_from_slice(&frame.payload_len.to_le_bytes());
    header[3..7].copy_from_slice(&frame.request_id.to_le_bytes());

    write_all(transport, &header)?;
    write_all(transport, frame.payload())?;
    Ok(())
}

pub fn read_frame<T: Read>(transport: &mut T) -> Result<ControlFrame, CodecError> {
    let mut buf = [0u8; MAX_WIRE_FRAME_LEN];
    read_exact(transport, &mut buf[..HEADER_LEN])?;
    let payload_len = u16::from_le_bytes([buf[1], buf[2]]) as usize;
    if payload_len > MAX_FRAME_PAYLOAD {
        return Err(CodecError::PayloadTooLarge { len: payload_len });
    }
    let total = HEADER_LEN + payload_len;
    read_exact(transport, &mut buf[HEADER_LEN..total])?;
    decode_frame_from_prefix(&buf[..total])
}

fn write_all<T: Write>(transport: &mut T, mut src: &[u8]) -> Result<(), CodecError> {
    while !src.is_empty() {
        match transport.write(src) {
            Ok(0) => return Err(CodecError::Closed),
            Ok(n) => src = &src[n..],
            Err(PipeError::WouldBlock) => relax_wait(),
        }
    }
    Ok(())
}

/// Incremental decoder for non-blocking transports.
///
/// Keeps partial bytes until a full frame is available; supports multiple
/// frames read in one chunk.
#[derive(Debug, Clone)]
pub struct FrameReadBuf {
    storage: [u8; MAX_WIRE_FRAME_LEN],
    len: usize,
}

impl Default for FrameReadBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameReadBuf {
    pub const fn new() -> Self {
        Self {
            storage: [0u8; MAX_WIRE_FRAME_LEN],
            len: 0,
        }
    }

    /// Append bytes from `transport` and return the next full frame, if any.
    ///
    /// Returns `Ok(None)` when starved by `WouldBlock` or a partial frame.
    pub fn try_read_frame<T: Read>(
        &mut self,
        transport: &mut T,
    ) -> Result<Option<ControlFrame>, CodecError> {
        loop {
            if self.len >= HEADER_LEN {
                let payload_len = u16::from_le_bytes([self.storage[1], self.storage[2]]) as usize;
                if payload_len > MAX_FRAME_PAYLOAD {
                    return Err(CodecError::PayloadTooLarge { len: payload_len });
                }
                let total = HEADER_LEN + payload_len;
                if total > MAX_WIRE_FRAME_LEN {
                    return Err(CodecError::PayloadTooLarge { len: payload_len });
                }
                if self.len >= total {
                    let frame = decode_frame_from_prefix(&self.storage[..total])?;
                    self.consume_prefix(total);
                    return Ok(Some(frame));
                }
            }
            if self.len >= self.storage.len() {
                // Malformed / oversized; avoid growing past the buffer.
                return Err(CodecError::PayloadTooLarge {
                    len: MAX_FRAME_PAYLOAD + 1,
                });
            }
            match transport.read(&mut self.storage[self.len..]) {
                Ok(0) => return Err(CodecError::Closed),
                Ok(n) => self.len += n,
                Err(PipeError::WouldBlock) => return Ok(None),
            }
        }
    }

    fn consume_prefix(&mut self, n: usize) {
        debug_assert!(n <= self.len);
        let remain = self.len - n;
        if remain > 0 {
            self.storage.copy_within(n..self.len, 0);
        }
        self.len = remain;
    }
}

fn decode_frame_from_prefix(buf: &[u8]) -> Result<ControlFrame, CodecError> {
    if buf.len() < HEADER_LEN {
        return Err(CodecError::PayloadTooLarge { len: 0 });
    }
    let message_type =
        MessageType::from_u8(buf[0]).ok_or(CodecError::UnknownMessageType(buf[0]))?;
    let payload_len = u16::from_le_bytes([buf[1], buf[2]]) as usize;
    if payload_len > MAX_FRAME_PAYLOAD {
        return Err(CodecError::PayloadTooLarge { len: payload_len });
    }
    let total = HEADER_LEN + payload_len;
    if buf.len() < total {
        return Err(CodecError::PayloadTooLarge { len: payload_len });
    }
    let request_id = u32::from_le_bytes([buf[3], buf[4], buf[5], buf[6]]);

    let mut frame = ControlFrame {
        message_type,
        request_id,
        payload_len: payload_len as u16,
        payload: [0u8; MAX_FRAME_PAYLOAD],
    };
    frame.payload[..payload_len].copy_from_slice(&buf[HEADER_LEN..total]);
    Ok(frame)
}

fn read_exact<T: Read>(transport: &mut T, mut dst: &mut [u8]) -> Result<(), CodecError> {
    while !dst.is_empty() {
        match transport.read(dst) {
            Ok(0) => return Err(CodecError::Closed),
            Ok(n) => dst = &mut dst[n..],
            Err(PipeError::WouldBlock) => relax_wait(),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessageType, MAX_FRAME_PAYLOAD};

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
    fn write_read_round_trip_empty_payload() {
        let mut pipe = MemPipe::<256>::new();
        let frame = ControlFrame::new(MessageType::ListenRequest, 7, &[]);
        write_frame(&mut pipe, &frame).unwrap();

        let got = read_frame(&mut pipe).unwrap();
        assert_eq!(got.message_type, MessageType::ListenRequest);
        assert_eq!(got.request_id, 7);
        assert_eq!(got.payload_len, 0);
    }

    #[test]
    fn write_read_round_trip_with_payload() {
        // The codec is payload-agnostic; verify framing identity with raw bytes.
        let pl: [u8; 6] = [127, 0, 0, 1, 0x90, 0x1f];
        let frame = ControlFrame::new(MessageType::ConnectRequest, 99, &pl);

        let mut pipe = MemPipe::<256>::new();
        write_frame(&mut pipe, &frame).unwrap();
        let got = read_frame(&mut pipe).unwrap();

        assert_eq!(got.message_type, MessageType::ConnectRequest);
        assert_eq!(got.request_id, 99);
        assert_eq!(got.payload(), &pl);
    }

    #[test]
    fn write_read_two_frames_in_order() {
        let mut pipe = MemPipe::<1024>::new();
        let a = ControlFrame::new(MessageType::ListenRequest, 1, &[1, 2, 3]);
        let b = ControlFrame::new(MessageType::ConnectRequest, 2, &[]);
        write_frame(&mut pipe, &a).unwrap();
        write_frame(&mut pipe, &b).unwrap();

        let got_a = read_frame(&mut pipe).unwrap();
        assert_eq!(got_a.request_id, 1);
        assert_eq!(got_a.payload(), &[1, 2, 3]);

        let got_b = read_frame(&mut pipe).unwrap();
        assert_eq!(got_b.message_type, MessageType::ConnectRequest);
        assert_eq!(got_b.request_id, 2);
        assert_eq!(got_b.payload_len, 0);
    }

    #[test]
    fn write_read_max_sized_payload() {
        let payload: [u8; MAX_FRAME_PAYLOAD] = core::array::from_fn(|i| i as u8);
        let frame = ControlFrame::new(MessageType::IncomingConnection, 0x1234_5678, &payload);

        let mut pipe = MemPipe::<{ HEADER_LEN + MAX_FRAME_PAYLOAD + 32 }>::new();
        write_frame(&mut pipe, &frame).unwrap();
        let got = read_frame(&mut pipe).unwrap();
        assert_eq!(got.payload_len as usize, MAX_FRAME_PAYLOAD);
        assert_eq!(got.payload(), payload.as_slice());
    }

    #[test]
    fn read_unknown_message_type() {
        let mut pipe = MemPipe::<32>::new();
        let mut header = [0u8; HEADER_LEN];
        header[0] = 99;
        header[1..3].copy_from_slice(&0u16.to_le_bytes());
        header[3..7].copy_from_slice(&1u32.to_le_bytes());
        pipe.write(&header).unwrap();
        let err = read_frame(&mut pipe).unwrap_err();
        assert_eq!(err, CodecError::UnknownMessageType(99));
    }

    #[test]
    fn frame_read_buf_decodes_back_to_back_frames() {
        let pay = 7u32.to_le_bytes();
        let a = ControlFrame::new(MessageType::AcceptRequest, 10, &pay);
        let b = ControlFrame::new(MessageType::ListenRequest, 20, &[]);
        let mut pipe = MemPipe::<1024>::new();
        write_frame(&mut pipe, &a).unwrap();
        write_frame(&mut pipe, &b).unwrap();
        pipe.pos = 0;

        let mut dec = FrameReadBuf::new();
        let f1 = dec.try_read_frame(&mut pipe).unwrap().unwrap();
        assert_eq!(f1.message_type, MessageType::AcceptRequest);
        assert_eq!(f1.request_id, 10);
        assert_eq!(f1.payload(), &pay);

        let f2 = dec.try_read_frame(&mut pipe).unwrap().unwrap();
        assert_eq!(f2.message_type, MessageType::ListenRequest);
        assert_eq!(f2.request_id, 20);

        assert!(dec.try_read_frame(&mut pipe).unwrap().is_none());
    }

    #[test]
    fn frame_read_buf_one_byte_at_a_time() {
        let pay = 3u32.to_le_bytes();
        let frame = ControlFrame::new(MessageType::AcceptRequest, 99, &pay);
        let mut wire = MemPipe::<256>::new();
        write_frame(&mut wire, &frame).unwrap();
        let encoded_len = wire.len;

        struct OneByte<'a> {
            buf: &'a [u8],
            pos: usize,
        }

        impl<'a> Read for OneByte<'a> {
            fn read(&mut self, dst: &mut [u8]) -> Result<usize, PipeError> {
                if self.pos >= self.buf.len() {
                    return Err(PipeError::WouldBlock);
                }
                dst[0] = self.buf[self.pos];
                self.pos += 1;
                Ok(1)
            }
        }

        let mut reader = OneByte {
            buf: &wire.buf[..encoded_len],
            pos: 0,
        };
        let mut dec = FrameReadBuf::new();
        let mut decoded = None;
        for _ in 0..4096 {
            if let Some(f) = dec.try_read_frame(&mut reader).unwrap() {
                decoded = Some(f);
                break;
            }
        }
        let got = decoded.expect("should decode after enough bytes");
        assert_eq!(got.message_type, MessageType::AcceptRequest);
        assert_eq!(got.request_id, 99);
        assert_eq!(got.payload(), &pay);
    }
}
