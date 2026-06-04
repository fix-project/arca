//! Semantic control messages: the layer above [`crate::codec`].
//!
//! [`crate::codec`] turns the wire bytestream into [`ControlFrame`]s
//! (header + raw payload); this module turns a frame into a parsed,
//! direction-typed message and back.
//!
//! - [`ControlRequest`]: Arca -> Linux monitor.
//! - [`ControlReply`]:   monitor -> Arca.
//!
//! Parsing is fallible (`ControlRequest::try_from(&frame)?`); encoding is
//! infallible (`req.to_frame()`). All payload encode/decode lives in this
//! file. Payload structs in [`crate::protocol`] are pure data.
//!
//! The split keeps framing (how many bytes is one message) independent
//! from semantics (what the message means), so the incremental decoder
//! in [`crate::codec`] never needs to understand payloads.

use crate::{
    CodecError, ConnectionReady, ControlFrame, DataPipeInfo, Endpoint, MessageType,
    MAX_FRAME_PAYLOAD,
};

/// A request flowing Arca -> Linux. `request_id` correlates each request
/// with the [`ControlReply`] that answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlRequest {
    /// `bind`+`listen` on `endpoint`.
    Listen { request_id: u32, endpoint: Endpoint },
    /// `connect` outbound to `endpoint`.
    Connect { request_id: u32, endpoint: Endpoint },
    /// Wait for the next inbound connection on `listener_id`.
    Accept { request_id: u32, listener_id: u32 },
}

impl ControlRequest {
    /// Correlation id shared with the matching [`ControlReply`].
    pub fn request_id(&self) -> u32 {
        match self {
            Self::Listen { request_id, .. }
            | Self::Connect { request_id, .. }
            | Self::Accept { request_id, .. } => *request_id,
        }
    }

    /// Encode into a ready-to-write [`ControlFrame`].
    pub fn to_frame(&self) -> ControlFrame {
        let mut pl = [0u8; MAX_FRAME_PAYLOAD];
        let (mt, n) = match self {
            Self::Listen { endpoint, .. } => (
                MessageType::ListenRequest,
                write_endpoint(&mut pl, endpoint),
            ),
            Self::Connect { endpoint, .. } => (
                MessageType::ConnectRequest,
                write_endpoint(&mut pl, endpoint),
            ),
            Self::Accept { listener_id, .. } => {
                (MessageType::AcceptRequest, write_u32(&mut pl, *listener_id))
            }
        };
        ControlFrame::new(mt, self.request_id(), &pl[..n])
    }
}

impl TryFrom<&ControlFrame> for ControlRequest {
    type Error = CodecError;

    fn try_from(f: &ControlFrame) -> Result<Self, CodecError> {
        let request_id = f.request_id;
        Ok(match f.message_type {
            MessageType::ListenRequest => Self::Listen {
                request_id,
                endpoint: read_endpoint(f.payload())?,
            },
            MessageType::ConnectRequest => Self::Connect {
                request_id,
                endpoint: read_endpoint(f.payload())?,
            },
            MessageType::AcceptRequest => Self::Accept {
                request_id,
                listener_id: read_u32(f.payload())?,
            },
            other => return Err(CodecError::UnexpectedMessage(other)),
        })
    }
}

/// A reply flowing Linux -> Arca, tagged with the originating `request_id`.
///
/// [`Self::AcceptOk`] is carried on the wire by [`MessageType::IncomingConnection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlReply {
    ListenOk {
        request_id: u32,
        listener_id: u32,
    },
    ListenErr {
        request_id: u32,
        code: u32,
    },
    ConnectOk {
        request_id: u32,
        ready: ConnectionReady,
    },
    ConnectErr {
        request_id: u32,
        code: u32,
    },
    AcceptOk {
        request_id: u32,
        ready: ConnectionReady,
    },
    /// Monitor couldn't fulfil an `AcceptRequest` (unknown listener, kernel
    /// accept failed). `code` is an errno-like value.
    AcceptErr {
        request_id: u32,
        code: u32,
    },
}

impl ControlReply {
    /// Correlation id of the request this reply answers.
    pub fn request_id(&self) -> u32 {
        match self {
            Self::ListenOk { request_id, .. }
            | Self::ListenErr { request_id, .. }
            | Self::ConnectOk { request_id, .. }
            | Self::ConnectErr { request_id, .. }
            | Self::AcceptOk { request_id, .. }
            | Self::AcceptErr { request_id, .. } => *request_id,
        }
    }

    /// The wire message type this reply encodes to.
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::ListenOk { .. } => MessageType::ListenOk,
            Self::ListenErr { .. } => MessageType::ListenErr,
            Self::ConnectOk { .. } => MessageType::ConnectOk,
            Self::ConnectErr { .. } => MessageType::ConnectErr,
            Self::AcceptOk { .. } => MessageType::IncomingConnection,
            Self::AcceptErr { .. } => MessageType::AcceptErr,
        }
    }

    /// Encode into a ready-to-write [`ControlFrame`].
    pub fn to_frame(&self) -> ControlFrame {
        let mut pl = [0u8; MAX_FRAME_PAYLOAD];
        let n = match self {
            Self::ListenOk { listener_id, .. } => write_u32(&mut pl, *listener_id),
            Self::ListenErr { code, .. }
            | Self::ConnectErr { code, .. }
            | Self::AcceptErr { code, .. } => write_u32(&mut pl, *code),
            Self::ConnectOk { ready, .. } | Self::AcceptOk { ready, .. } => {
                write_ready(&mut pl, ready)
            }
        };
        ControlFrame::new(self.message_type(), self.request_id(), &pl[..n])
    }
}

impl TryFrom<&ControlFrame> for ControlReply {
    type Error = CodecError;

    fn try_from(f: &ControlFrame) -> Result<Self, CodecError> {
        let request_id = f.request_id;
        Ok(match f.message_type {
            MessageType::ListenOk => Self::ListenOk {
                request_id,
                listener_id: read_u32(f.payload())?,
            },
            MessageType::ListenErr => Self::ListenErr {
                request_id,
                code: read_u32(f.payload())?,
            },
            MessageType::ConnectOk => Self::ConnectOk {
                request_id,
                ready: read_ready(f.payload())?,
            },
            MessageType::ConnectErr => Self::ConnectErr {
                request_id,
                code: read_u32(f.payload())?,
            },
            MessageType::IncomingConnection => Self::AcceptOk {
                request_id,
                ready: read_ready(f.payload())?,
            },
            MessageType::AcceptErr => Self::AcceptErr {
                request_id,
                code: read_u32(f.payload())?,
            },
            other => return Err(CodecError::UnexpectedMessage(other)),
        })
    }
}

// --- payload codec helpers (the only place bytes meet semantic types) ---

fn take(p: &[u8], n: usize) -> Result<&[u8], CodecError> {
    p.get(..n).ok_or(CodecError::ShortPayload {
        expected: n,
        got: p.len(),
    })
}

fn read_u32(p: &[u8]) -> Result<u32, CodecError> {
    Ok(u32::from_le_bytes(take(p, 4)?.try_into().unwrap()))
}

fn read_endpoint(p: &[u8]) -> Result<Endpoint, CodecError> {
    let p = take(p, 6)?;
    Ok(Endpoint::new(
        [p[0], p[1], p[2], p[3]],
        u16::from_le_bytes([p[4], p[5]]),
    ))
}

fn read_ready(p: &[u8]) -> Result<ConnectionReady, CodecError> {
    let p = take(p, 24)?;
    Ok(ConnectionReady {
        listener_id: u32::from_le_bytes(p[0..4].try_into().unwrap()),
        connection_id: u32::from_le_bytes(p[4..8].try_into().unwrap()),
        pipe: DataPipeInfo::new(
            u64::from_le_bytes(p[8..16].try_into().unwrap()),
            u64::from_le_bytes(p[16..24].try_into().unwrap()),
        ),
    })
}

fn write_u32(out: &mut [u8], v: u32) -> usize {
    out[..4].copy_from_slice(&v.to_le_bytes());
    4
}

fn write_endpoint(out: &mut [u8], ep: &Endpoint) -> usize {
    out[..4].copy_from_slice(&ep.host);
    out[4..6].copy_from_slice(&ep.port.to_le_bytes());
    6
}

fn write_ready(out: &mut [u8], r: &ConnectionReady) -> usize {
    out[..4].copy_from_slice(&r.listener_id.to_le_bytes());
    out[4..8].copy_from_slice(&r.connection_id.to_le_bytes());
    out[8..16].copy_from_slice(&r.pipe.shm_offset.to_le_bytes());
    out[16..24].copy_from_slice(&r.pipe.ring_size.to_le_bytes());
    24
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ready() -> ConnectionReady {
        ConnectionReady {
            listener_id: 3,
            connection_id: 9,
            pipe: DataPipeInfo::new(9, 128),
        }
    }

    #[test]
    fn request_round_trip_through_frame() {
        for req in [
            ControlRequest::Listen {
                request_id: 1,
                endpoint: Endpoint::new([127, 0, 0, 1], 8080),
            },
            ControlRequest::Connect {
                request_id: 2,
                endpoint: Endpoint::new([8, 8, 8, 8], 443),
            },
            ControlRequest::Accept {
                request_id: 3,
                listener_id: 77,
            },
        ] {
            let frame = req.to_frame();
            assert_eq!(frame.request_id, req.request_id());
            assert_eq!(ControlRequest::try_from(&frame).unwrap(), req);
        }
    }

    #[test]
    fn reply_round_trip_through_frame() {
        for reply in [
            ControlReply::ListenOk {
                request_id: 1,
                listener_id: 5,
            },
            ControlReply::ListenErr {
                request_id: 2,
                code: 98,
            },
            ControlReply::ConnectOk {
                request_id: 3,
                ready: sample_ready(),
            },
            ControlReply::ConnectErr {
                request_id: 4,
                code: 111,
            },
            ControlReply::AcceptOk {
                request_id: 5,
                ready: sample_ready(),
            },
            ControlReply::AcceptErr {
                request_id: 6,
                code: 9,
            },
        ] {
            let frame = reply.to_frame();
            assert_eq!(frame.request_id, reply.request_id());
            assert_eq!(frame.message_type, reply.message_type());
            assert_eq!(ControlReply::try_from(&frame).unwrap(), reply);
        }
    }

    #[test]
    fn accept_ok_uses_incoming_connection_wire_type() {
        let f = ControlReply::AcceptOk {
            request_id: 7,
            ready: sample_ready(),
        }
        .to_frame();
        assert_eq!(f.message_type, MessageType::IncomingConnection);
    }

    #[test]
    fn rejects_wrong_direction_message_type() {
        let req_frame = ControlRequest::Accept {
            request_id: 1,
            listener_id: 2,
        }
        .to_frame();
        assert_eq!(
            ControlReply::try_from(&req_frame),
            Err(CodecError::UnexpectedMessage(MessageType::AcceptRequest))
        );
        let reply_frame = ControlReply::ListenOk {
            request_id: 1,
            listener_id: 5,
        }
        .to_frame();
        assert_eq!(
            ControlRequest::try_from(&reply_frame),
            Err(CodecError::UnexpectedMessage(MessageType::ListenOk))
        );
    }

    #[test]
    fn try_from_short_payload_errors() {
        let mut frame = ControlRequest::Listen {
            request_id: 1,
            endpoint: Endpoint::new([0, 0, 0, 0], 0),
        }
        .to_frame();
        frame.payload_len = 3;
        assert_eq!(
            ControlRequest::try_from(&frame),
            Err(CodecError::ShortPayload {
                expected: 6,
                got: 3
            })
        );
    }
}
