#![allow(unused)]

//! Arca-side wrappers over the control pipe.
//!
use crate::prelude::*;

use common::message::control::Error as CodecError;
use common::message::control::NewPipeReply;
use common::message::control::NewPipeRequest;
use common::pipe::{BidirectionalPipe, Read, SharedMemoryRegion, Write, ARCA_SIDE};
use ouroboros::self_referencing;

use crate::doorbell::VMToHostDoorBell;
use crate::doorbell::VMToHostDoorBellInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Codec(CodecError),
}

impl From<CodecError> for Error {
    fn from(value: CodecError) -> Self {
        Self::Codec(value)
    }
}

type Request = NewPipeRequest;
type Reply = NewPipeReply<VMToHostDoorBellInfo>;
type ArcaPipe<'a> = BidirectionalPipe<'a, VMToHostDoorBell>;

#[self_referencing]
struct ArcaPipeWrapper {
    shm: SharedMemoryRegion,

    #[borrows(shm)]
    #[covariant]
    pipe: ArcaPipe<'this>,
}

fn new_pipe_from_reply(reply: Reply) -> ArcaPipeWrapper {
    let shm_ptr: *mut u8 = BuddyAllocator.from_offset(reply.pipe_info.shm_offset as usize);
    let ring_size = reply.pipe_info.ring_size;
    let shm_len = ArcaPipe::required_size(ring_size);
    let shm = unsafe { SharedMemoryRegion::from_raw(shm_ptr, shm_len) };
    let read_available_doorbell =
        VMToHostDoorBell::from_door_bell_info(reply.read_available_door_bell_info);
    let write_available_doorbell =
        VMToHostDoorBell::from_door_bell_info(reply.write_available_door_bell_info);

    ArcaPipeWrapperBuilder {
        shm,
        pipe_builder: |shm_ref: &SharedMemoryRegion| {
            ArcaPipe::new(
                shm_ref,
                ring_size,
                ARCA_SIDE,
                read_available_doorbell,
                write_available_doorbell,
            )
        },
    }
    .build()
}

/// Owner of the **single** control pipe on the Arca side.
pub struct ArcaSession<'a, T: Read + Write> {
    transport: &'a mut T,
}

impl<'a, T: Read + Write> ArcaSession<'a, T> {
    pub fn new(transport: &'a mut T) -> Self {
        Self { transport }
    }
}
