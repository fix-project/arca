#![allow(unused)]

use crate::{kthread, prelude::*};

use common::message::frame_codec::Error;
use common::message::frame_codec::{Frame, FrameReadBuf, FrameWriteBuf};
use common::message::traits::FixedMsg;
use common::pipe::{BidirectionalPipe, Read, SharedMemoryRegion, Write, ARCA_SIDE};
use ouroboros::self_referencing;

use crate::control_pipe::Reply;
use crate::doorbell::VMToHostDoorBell;

type ArcaPipe<'a> = BidirectionalPipe<'a, VMToHostDoorBell>;

#[self_referencing]
pub struct ArcaPipeWrapper {
    shm: SharedMemoryRegion,
    pipe_id: u64,

    #[borrows(shm)]
    #[covariant]
    pipe: ArcaPipe<'this>,
}

impl ArcaPipeWrapper {
    pub fn new_pipe(reply: Reply) -> Self {
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
            pipe_id: reply.pipe_info.pipe_id,
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
}

// TODO: we may need to eventually support splitting into individual read end and write end
pub fn write_one_frame(
    pipe: &mut ArcaPipeWrapper,
    frame_writer: &mut FrameWriteBuf,
) -> Result<(), Error> {
    loop {
        let res = pipe.with_pipe_mut(|a| frame_writer.try_write_frame(a));
        match res {
            Err(e) => return Err(e),
            Ok(false) => kthread::wfi(),
            Ok(true) => return Ok(()),
        }
    }
}

pub fn read_one_frame<const MAX_FRAME_PAYLOAD: usize>(
    pipe: &mut ArcaPipeWrapper,
    frame_reader: &mut FrameReadBuf<MAX_FRAME_PAYLOAD>,
) -> Result<Frame<MAX_FRAME_PAYLOAD>, Error> {
    loop {
        let res = pipe.with_pipe_mut(|a| frame_reader.try_read_frame(a));
        match res {
            Err(e) => return Err(e),
            Ok(None) => kthread::wfi(),
            Ok(Some(f)) => return Ok(f),
        }
    }
}
