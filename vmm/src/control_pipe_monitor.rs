#![allow(unused)]

//! Linux-side **monitor** of the control pipe
//!
use crate::doorbell::{
    new_vm_to_host_door_bell, HostToVMDoorBell, VMToHostDoorBellInfo, VMToHostDoorBellWaiter,
};
use common::{
    message::{
        control::{DataPipeInfo, Error as CodecError, NewPipeReply, NewPipeRequest},
        traits::FixedMsg,
    },
    pipe::{BidirectionalPipe, SharedMemoryRegion, HOST_SIDE},
    BuddyAllocator,
};
use kvm_ioctls::{IoEventAddress, VmFd};
use ouroboros::self_referencing;

#[derive(Debug)]
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
type VmmPipe<'a> = BidirectionalPipe<'a, HostToVMDoorBell>;

#[self_referencing]
struct VmmPipeWrapper {
    shm: SharedMemoryRegion,

    #[borrows(shm)]
    #[covariant]
    pipe: VmmPipe<'this>,
}

const REQ_FRAME_SIZE: usize = Request::SIZE;
const REPLY_FRAME_SIZE: usize = Reply::SIZE;
const HOST_TO_VM_GSI: u32 = 2;

fn new_host_to_vm_door_bell(vm: &VmFd) -> HostToVMDoorBell {
    HostToVMDoorBell::new(vm, HOST_TO_VM_GSI)
}

struct VMToHostDoorBellPair {
    read_available_bell_info: VMToHostDoorBellInfo,
    write_available_bell_info: VMToHostDoorBellInfo,
    read_bell_waiter: VMToHostDoorBellWaiter,
    write_bell_waiter: VMToHostDoorBellWaiter,
}

fn new_vm_to_host_door_bell_pair(
    vm: &VmFd,
    addr: IoEventAddress,
    pipe_id: u64,
) -> VMToHostDoorBellPair {
    assert!(pipe_id < u64::MAX / 2);
    let (read_available_bell_info, read_bell_waiter) =
        new_vm_to_host_door_bell(vm, addr, pipe_id * 2);
    let (write_available_bell_info, write_bell_waiter) =
        new_vm_to_host_door_bell(vm, addr, pipe_id * 2 + 1);
    VMToHostDoorBellPair {
        read_available_bell_info,
        write_available_bell_info,
        read_bell_waiter,
        write_bell_waiter,
    }
}

fn new_pipe(
    vm: &VmFd,
    addr: IoEventAddress,
    ring_size: u64,
    pipe_id: u64,
) -> (
    VmmPipeWrapper,
    VMToHostDoorBellWaiter,
    VMToHostDoorBellWaiter,
    Reply,
) {
    let host_read_door_bell = new_host_to_vm_door_bell(vm);
    let host_write_door_bell = new_host_to_vm_door_bell(vm);

    let vm_to_host = new_vm_to_host_door_bell_pair(vm, addr, pipe_id);

    let shm_len = VmmPipe::required_size(ring_size) as usize;
    let mut shm = Vec::with_capacity_in(shm_len, BuddyAllocator);
    shm.resize(shm_len, 0u8);
    let (shm, _) = Box::into_raw_with_allocator(shm.into_boxed_slice());
    let shm_ptr = shm as *mut u8;

    let vmm_pipe = VmmPipeWrapperBuilder {
        shm: unsafe { SharedMemoryRegion::from_raw(shm_ptr, shm_len as u64) },
        pipe_builder: |shm_ref: &SharedMemoryRegion| {
            VmmPipe::new(
                shm_ref,
                ring_size,
                HOST_SIDE,
                host_read_door_bell,
                host_write_door_bell,
            )
        },
    }
    .build();

    let pipe_info = DataPipeInfo {
        shm_offset: BuddyAllocator.to_offset(shm_ptr) as u64,
        ring_size,
        pipe_id,
    };
    let reply = Reply {
        pipe_info,
        read_available_door_bell_info: vm_to_host.read_available_bell_info,
        write_available_door_bell_info: vm_to_host.write_available_bell_info,
    };

    (
        vmm_pipe,
        vm_to_host.read_bell_waiter,
        vm_to_host.write_bell_waiter,
        reply,
    )
}

/// Linux-side state machine.
pub struct Monitor {
    vm: VmFd,
    addr: IoEventAddress,
    next_stream_id: u64,
}

impl Monitor {
    pub fn new(vm: VmFd, addr: IoEventAddress) -> Self {
        Self {
            vm,
            addr,
            // 0 is reserved for the control pipe
            next_stream_id: 1,
        }
    }

    /// Translate one Arca → Linux request frame into the reply we owe Arca.
    pub fn dispatch_request(&mut self, req: Request) -> Result<Reply, Error> {
        let (vmm_pipe, read_available_bell, write_available_bell, reply) =
            new_pipe(&self.vm, self.addr, req.ring_size, self.next_stream_id);
        self.next_stream_id += 1;
        Ok(reply)
    }
}
