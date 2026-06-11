//! Linux-side **monitor** of the control pipe
//!
use common::message::{
    control::{NewPipeReply, NewPipeRequest},
    traits::FixedMsg,
};
use kvm_ioctls::{IoEventAddress, VmFd};

use crate::doorbell::VMToHostDoorBellInfo;
use crate::{monitor::StatefulMonitor, vmm_pipe::new_pipe};

use crate::monitor::Error as MonitorError;
use futures::Future;

const MAX_FRAME_PAYLOAD: usize = NewPipeRequest::SIZE;

/// Linux-side state machine.
struct ControlMonitorState {
    vm: VmFd,
    addr: IoEventAddress,
    next_stream_id: u64,
}

impl StatefulMonitor<MAX_FRAME_PAYLOAD> for ControlMonitorState {
    type Request = NewPipeRequest;
    type Reply = NewPipeReply<VMToHostDoorBellInfo>;

    fn handle_request(
        &mut self,
        req: Self::Request,
    ) -> impl Future<Output = Result<Self::Reply, MonitorError>> + Send {
        let (_vmm_pipe, _read_available_bell, _write_available_bell, reply) =
            new_pipe(&self.vm, self.addr, req.ring_size, self.next_stream_id);
        self.next_stream_id += 1;

        async move { Ok(reply) }
    }
}
