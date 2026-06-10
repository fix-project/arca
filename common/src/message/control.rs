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

use crate::message::traits::Error;

use crate::message::frame_codec::Frame;
use crate::message::traits::FixedMsg;

impl FixedMsg for u64 {
    const SIZE: usize = 8;

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != Self::SIZE {
            Err(Error::SerializerError)
        } else {
            out.copy_from_slice(&self.to_le_bytes());
            Ok(())
        }
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != Self::SIZE {
            Err(Error::ParserError)
        } else {
            Ok(u64::from_le_bytes(
                input.try_into().map_err(|_| Error::ParserError)?,
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewPipeRequest {
    pub ring_size: u64,
}

impl FixedMsg for NewPipeRequest {
    const SIZE: usize = u64::SIZE;

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        self.ring_size.encode(out)
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        let ring_size = u64::decode(input)?;
        Ok(Self { ring_size })
    }
}

impl<const MAX_FRAME_PAYLOAD: usize> TryFrom<&Frame<MAX_FRAME_PAYLOAD>> for NewPipeRequest {
    type Error = Error;

    fn try_from(f: &Frame<MAX_FRAME_PAYLOAD>) -> Result<Self, Error> {
        if MAX_FRAME_PAYLOAD < Self::SIZE {
            Err(Error::ParserError)
        } else {
            Self::decode(f.as_slice())
        }
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
    pub pipe_id: u64,
}

impl DataPipeInfo {
    pub fn new(shm_offset: u64, ring_size: u64, pipe_id: u64) -> Self {
        Self {
            shm_offset,
            ring_size,
            pipe_id,
        }
    }
}

impl FixedMsg for DataPipeInfo {
    const SIZE: usize = u64::SIZE + u64::SIZE + u64::SIZE;

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != Self::SIZE {
            Err(Error::SerializerError)
        } else {
            let (buf, rest) = out.split_at_mut(u64::SIZE);
            self.shm_offset.encode(buf)?;
            let (buf, rest) = rest.split_at_mut(u64::SIZE);
            self.ring_size.encode(buf)?;
            self.pipe_id.encode(rest)
        }
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != Self::SIZE {
            Err(Error::ParserError)
        } else {
            let (buf, rest) = input.split_at(u64::SIZE);
            let shm_offset = u64::decode(buf)?;
            let (buf, rest) = rest.split_at(u64::SIZE);
            let ring_size = u64::decode(buf)?;
            let pipe_id = u64::decode(rest)?;

            Ok(Self {
                shm_offset,
                ring_size,
                pipe_id,
            })
        }
    }
}

/// How Arca finds the per-connection data pipe.
///
/// Layout on the wire (16 bytes): `shm_offset` (u64 LE) then `ring_size` (u64 LE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewPipeReply<DoorBellInfo: FixedMsg> {
    pub pipe_info: DataPipeInfo,
    pub read_available_door_bell_info: DoorBellInfo,
    pub write_available_door_bell_info: DoorBellInfo,
}

impl<DoorBellInfo: FixedMsg> FixedMsg for NewPipeReply<DoorBellInfo> {
    const SIZE: usize = DataPipeInfo::SIZE + DoorBellInfo::SIZE + DoorBellInfo::SIZE;

    fn encode(&self, out: &mut [u8]) -> Result<(), Error> {
        if out.len() != Self::SIZE {
            Err(Error::SerializerError)
        } else {
            let (buf, rest) = out.split_at_mut(DataPipeInfo::SIZE);
            self.pipe_info.encode(buf)?;
            let (buf, rest) = rest.split_at_mut(DoorBellInfo::SIZE);
            self.read_available_door_bell_info.encode(buf)?;
            self.write_available_door_bell_info.encode(rest)
        }
    }

    fn decode(input: &[u8]) -> Result<Self, Error> {
        if input.len() != Self::SIZE {
            Err(Error::ParserError)
        } else {
            let (buf, rest) = input.split_at(DataPipeInfo::SIZE);
            let pipe_info = DataPipeInfo::decode(buf)?;
            let (buf, rest) = rest.split_at(DoorBellInfo::SIZE);
            let read_available_door_bell_info = DoorBellInfo::decode(buf)?;
            let write_available_door_bell_info = DoorBellInfo::decode(rest)?;

            Ok(Self {
                pipe_info,
                read_available_door_bell_info,
                write_available_door_bell_info,
            })
        }
    }
}

impl<const MAX_FRAME_PAYLOAD: usize, DoorBellInfo: FixedMsg> TryFrom<&Frame<MAX_FRAME_PAYLOAD>>
    for NewPipeReply<DoorBellInfo>
{
    type Error = Error;

    fn try_from(f: &Frame<MAX_FRAME_PAYLOAD>) -> Result<Self, Error> {
        if MAX_FRAME_PAYLOAD < Self::SIZE {
            Err(Error::ParserError)
        } else {
            Self::decode(f.as_slice())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestDoorBellInfo = u64;
    type TestPipeReply = NewPipeReply<TestDoorBellInfo>;
    const TEST_SIZE: usize = TestPipeReply::SIZE;

    fn sample_reply() -> TestPipeReply {
        let pipe_info = DataPipeInfo {
            shm_offset: 10086,
            ring_size: 7474747,
            pipe_id: 201199,
        };

        TestPipeReply {
            pipe_info,
            read_available_door_bell_info: 42,
            write_available_door_bell_info: 1024,
        }
    }

    #[test]
    fn round_trip_through_frame() {
        let reply = sample_reply();

        let mut frame: Frame<256> = Frame {
            payload_len: TEST_SIZE,
            payload: [0u8; 256],
        };

        reply.encode(&mut frame.payload[..TEST_SIZE]).unwrap();
        assert_eq!(TestPipeReply::try_from(&frame).unwrap(), reply);
    }

    #[test]
    fn try_from_short_payload_errors() {
        let reply = sample_reply();

        let mut frame: Frame<256> = Frame {
            payload_len: TEST_SIZE,
            payload: [0u8; 256],
        };

        reply.encode(&mut frame.payload[..TEST_SIZE]).unwrap();
        frame.payload_len = 3;

        assert_eq!(TestPipeReply::try_from(&frame), Err(Error::ParserError));
    }
}
