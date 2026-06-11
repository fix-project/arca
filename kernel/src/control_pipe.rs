#![allow(unused)]

//! Arca-side wrappers over the control pipe.
//!
use common::message::control::NewPipeReply;
use common::message::control::NewPipeRequest;
use common::message::frame_codec::Error as FrameCodecError;
use common::message::frame_codec::{Frame, FrameReadBuf, FrameWriteBuf};
use common::message::traits::Error as MessageCodecError;
use common::message::traits::FixedMsg;
use common::message::traits::IntoBytes;

use crate::arca_pipe::ArcaPipeWrapper;
use crate::doorbell::VMToHostDoorBellInfo;

use crate::arca_pipe::read_one_frame;
use crate::arca_pipe::write_one_frame;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    FrameCodec(FrameCodecError),
    MessageCodec(MessageCodecError),
}

impl From<MessageCodecError> for Error {
    fn from(value: MessageCodecError) -> Self {
        Self::MessageCodec(value)
    }
}

impl From<FrameCodecError> for Error {
    fn from(value: FrameCodecError) -> Self {
        Self::FrameCodec(value)
    }
}

type Request = NewPipeRequest;
pub type Reply = NewPipeReply<VMToHostDoorBellInfo>;
const MAX_FRAME_PAYLOAD: usize = Reply::SIZE;

/// Owner of the **single** control pipe on the Arca side.
pub struct ArcaSession {
    transport: ArcaPipeWrapper,
    frame_writer: FrameWriteBuf,
    frame_reader: FrameReadBuf<MAX_FRAME_PAYLOAD>,
}

impl ArcaSession {
    pub fn new(transport: ArcaPipeWrapper) -> Self {
        Self {
            transport,
            frame_writer: FrameWriteBuf::default(),
            frame_reader: FrameReadBuf::default(),
        }
    }

    pub fn new_stream(&mut self, ring_size: u64) -> Result<ArcaPipeWrapper, Error> {
        let request = Request { ring_size };
        let frame = request.into_boxed_slice();

        self.frame_writer.load(frame).unwrap();
        write_one_frame(&mut self.transport, &mut self.frame_writer)?;

        let reply = read_one_frame(&mut self.transport, &mut self.frame_reader)?;
        let reply = Reply::try_from(&reply)?;
        Ok(ArcaPipeWrapper::new_pipe(reply))
    }
}
