//! Wire-protocol data types for the **single control pipe** between Arca
//! and the Linux monitor.
//!
//! This module is **pure data**: the framing/encoding live in
//! [`crate::codec`] and [`crate::message`]. The frame is the only thing
//! that flows across the control pipe.
//!
//! ```text
//! offset  size  field
//! ------  ----  -----------------------------------------
//!  0      1     message_type (u8, see MessageType)
//!  1      2     payload_len  (u16 little-endian, bytes)
//!  3      4     request_id   (u32 little-endian)
//!  7      ..    payload      (payload_len bytes; fixed-layout little-endian)
//! ```
//!
//! Per-connection bytestreams flow on **separate** shared-memory data pipes;
//! the receiver finds them via [`DataPipeInfo`] carried in the
//! [`crate::ControlReply::ConnectOk`] / [`crate::ControlReply::AcceptOk`]
//! payloads.

use arca_pipe::BidirectionalPipe;

/// Maximum payload bytes after the 7-byte header.
///
/// Sized comfortably above today's largest payload (`ConnectionReady`, 24 B)
/// so we have headroom to add fields without bumping a version byte.
pub const MAX_FRAME_PAYLOAD: usize = 256;

/// Errors from moving frames over a transport and from parsing them into
/// semantically meaningful messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// Frame header named a message-type byte we don't recognize.
    UnknownMessageType(u8),
    /// Declared payload length exceeds [`MAX_FRAME_PAYLOAD`].
    PayloadTooLarge { len: usize },
    /// Transport returned `Ok(0)` — the peer hung up.
    Closed,
    /// Payload was shorter than the fixed layout its message requires.
    ShortPayload { expected: usize, got: usize },
    /// Message type is valid on the wire but illegal in this direction
    /// (e.g. a reply parsed as a request, or vice versa).
    UnexpectedMessage(MessageType),
}

/// Catalog of message kinds carried on the control pipe.
///
/// Keep this list **small** and add new variants only when there's a real
/// need. Each variant is a single byte on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    ListenRequest = 1,
    ListenOk = 2,
    ConnectRequest = 3,
    ConnectOk = 4,
    /// Reply to [`MessageType::AcceptRequest`]; payload is `ConnectionReady`.
    IncomingConnection = 5,
    ListenErr = 6,
    ConnectErr = 7,
    AcceptRequest = 8,
    /// Linux → Arca: reply to [`MessageType::AcceptRequest`] when the
    /// monitor couldn't fulfil it (unknown listener, kernel accept failed).
    AcceptErr = 9,
}

impl MessageType {
    /// Reverse of `as u8`, returning `None` for unknown bytes so the codec
    /// can reject garbage instead of silently misinterpreting it.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::ListenRequest),
            2 => Some(Self::ListenOk),
            3 => Some(Self::ConnectRequest),
            4 => Some(Self::ConnectOk),
            5 => Some(Self::IncomingConnection),
            6 => Some(Self::ListenErr),
            7 => Some(Self::ConnectErr),
            8 => Some(Self::AcceptRequest),
            9 => Some(Self::AcceptErr),
            _ => None,
        }
    }
}

/// One control message in memory (header + inline payload buffer).
///
/// The `payload` array is fixed-size so `ControlFrame` is `Copy` and lives
/// happily in `no_std`. Only the first `payload_len` bytes are valid; use
/// [`ControlFrame::payload`] to get the live slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlFrame {
    pub message_type: MessageType,
    pub request_id: u32,
    pub payload_len: u16,
    pub payload: [u8; MAX_FRAME_PAYLOAD],
}

impl ControlFrame {
    /// Build a frame, copying `payload` into the inline buffer.
    ///
    /// Panics if `payload.len() > MAX_FRAME_PAYLOAD` — callers always own
    /// the payload, so this is a programming bug, not runtime input.
    pub fn new(message_type: MessageType, request_id: u32, payload: &[u8]) -> Self {
        assert!(payload.len() <= MAX_FRAME_PAYLOAD, "payload too large");
        let mut out = Self {
            message_type,
            request_id,
            payload_len: payload.len() as u16,
            payload: [0u8; MAX_FRAME_PAYLOAD],
        };
        out.payload[..payload.len()].copy_from_slice(payload);
        out
    }

    /// The valid prefix of `self.payload`.
    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len as usize]
    }
}

/// IPv4 host + port. Six bytes on the wire: 4 octets then `port` LE.
///
/// IPv6 isn't modeled yet; when we need it we'll add a sibling type or a
/// length-prefixed address field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    pub host: [u8; 4],
    pub port: u16,
}

impl Endpoint {
    pub fn new(host: [u8; 4], port: u16) -> Self {
        Self { host, port }
    }
}

/// How Arca finds the per-connection data pipe.
///
/// Layout on the wire (16 bytes): `shm_offset` (u64 LE) then `ring_size` (u64 LE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataPipeInfo {
    /// BuddyAllocator offset from the allocator base to the SHM region backing
    /// this pipe. Pass to `BuddyAllocator.from_offset()` to get a pointer.
    pub shm_offset: u64,
    /// Per-direction ring capacity in bytes (same value passed to
    /// [`BidirectionalPipe::new`]).
    pub ring_size: u64,
}

impl DataPipeInfo {
    pub fn new(shm_offset: u64, ring_size: u64) -> Self {
        Self { shm_offset, ring_size }
    }

    /// Total shared-memory bytes the receiver must map for this pipe.
    pub fn shm_len(self) -> u64 {
        BidirectionalPipe::required_size(self.ring_size)
    }
}

/// Payload of [`crate::ControlReply::ConnectOk`] (outbound) and
/// [`crate::ControlReply::AcceptOk`] (inbound). Same fields, same layout —
/// the message kind says which is which.
///
/// `listener_id == 0` means "outbound connection, no listener was involved".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionReady {
    pub listener_id: u32,
    pub connection_id: u32,
    pub pipe: DataPipeInfo,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_from_u8_round_trip() {
        for mt in [
            MessageType::ListenRequest,
            MessageType::ListenOk,
            MessageType::ConnectRequest,
            MessageType::ConnectOk,
            MessageType::IncomingConnection,
            MessageType::ListenErr,
            MessageType::ConnectErr,
            MessageType::AcceptRequest,
            MessageType::AcceptErr,
        ] {
            assert_eq!(MessageType::from_u8(mt as u8), Some(mt));
        }
    }

    #[test]
    fn message_type_from_u8_unknown() {
        assert_eq!(MessageType::from_u8(0), None);
        assert_eq!(MessageType::from_u8(10), None);
        assert_eq!(MessageType::from_u8(255), None);
    }

    #[test]
    fn control_frame_payload_slice() {
        let frame = ControlFrame::new(MessageType::ListenRequest, 0, &[10, 20]);
        assert_eq!(frame.payload(), &[10, 20]);
    }

    #[test]
    fn data_pipe_info_shm_len_matches_bidirectional_pipe() {
        let ring_size = 64u64;
        let info = DataPipeInfo::new(1, ring_size);
        assert_eq!(info.shm_len(), BidirectionalPipe::required_size(ring_size));
    }
}
