#![allow(unused)]

use common::{
    message::traits::FixedMsg,
    pipe::{DoorBell, DoorBellWaiter, PipeError},
    BuddyAllocator,
};
use kvm_ioctls::{IoEventAddress, VmFd};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

pub struct HostToVMDoorBell {
    fd: EventFd,
}

impl HostToVMDoorBell {
    pub fn new(vm: &VmFd, gsi: u32) -> Self {
        let evtfd = EventFd::new(EFD_NONBLOCK).unwrap();
        vm.register_irqfd(&evtfd, gsi)
            .expect("Failed to register irqfd");
        Self { fd: evtfd }
    }
}

impl DoorBell for HostToVMDoorBell {
    fn ring(&self) {
        while self.fd.write(1).is_err() {}
    }
}

pub struct VMToHostDoorBellWaiter {
    fd: EventFd,
}

impl VMToHostDoorBellWaiter {
    /// Each eventfd needs to have a unique {addr, datamatch} pair, and it is
    /// allowed to have multiple eventfds registered at the same address with
    /// different datamatch. The caller needs to guarantee that {addr, datamatch}
    /// hasn't been registered before
    fn new(vm: &VmFd, addr: &IoEventAddress, datamatch: u64) -> Self {
        let evtfd = EventFd::new(EFD_NONBLOCK).unwrap();
        vm.register_ioevent(&evtfd, addr, datamatch)
            .expect("Failed to register ioevent");
        Self { fd: evtfd }
    }
}

impl DoorBellWaiter for VMToHostDoorBellWaiter {
    fn wait(&mut self) {
        while let Err(e) = self.fd.read() {}
    }
}

struct VMToHostDoorBell {
    addr: IoEventAddress,
    datamatch: u64,
}

pub struct VMToHostDoorBellInfo {
    inner: (u64, u64),
}

impl VMToHostDoorBell {
    fn new(addr: IoEventAddress, datamatch: u64) -> Self {
        Self { addr, datamatch }
    }

    fn into_raw_parts(self) -> VMToHostDoorBellInfo {
        let addr = match self.addr {
            IoEventAddress::Pio(_) => todo!(),
            IoEventAddress::Mmio(addr) => addr,
        };

        let addr = BuddyAllocator.to_offset(addr as usize as *const ());

        VMToHostDoorBellInfo {
            inner: (addr as u64, self.datamatch),
        }
    }
}

impl FixedMsg for VMToHostDoorBellInfo {
    const SIZE: usize = u64::SIZE + u64::SIZE;

    fn encode(&self, out: &mut [u8]) -> Result<(), common::message::traits::Error> {
        if out.len() != Self::SIZE {
            Err(common::message::traits::Error::SerializerError)
        } else {
            let (a, b) = out.split_at_mut(u64::SIZE);
            self.inner.0.encode(a)?;
            self.inner.1.encode(b)
        }
    }

    fn decode(input: &[u8]) -> Result<Self, common::message::traits::Error> {
        if input.len() != Self::SIZE {
            Err(common::message::traits::Error::ParserError)
        } else {
            let (a, b) = input.split_at(u64::SIZE);
            Ok(Self {
                inner: (u64::decode(a)?, u64::decode(b)?),
            })
        }
    }
}

pub fn new_vm_to_host_door_bell(
    vm: &VmFd,
    addr: IoEventAddress,
    datamatch: u64,
) -> (VMToHostDoorBellInfo, VMToHostDoorBellWaiter) {
    let doorbellwaiter = VMToHostDoorBellWaiter::new(vm, &addr, datamatch);
    let doorbell = VMToHostDoorBell::new(addr, datamatch).into_raw_parts();
    (doorbell, doorbellwaiter)
}
