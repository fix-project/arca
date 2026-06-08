#![allow(unused)]

use common::{
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

pub struct VMToHostDoorBell {
    addr: IoEventAddress,
    datamatch: u64,
}

impl VMToHostDoorBell {
    fn new(addr: IoEventAddress, datamatch: u64) -> Self {
        Self { addr, datamatch }
    }

    fn into_raw_parts(self) -> (u64, u64) {
        let addr = match self.addr {
            IoEventAddress::Pio(_) => todo!(),
            IoEventAddress::Mmio(addr) => addr,
        };

        let addr = BuddyAllocator.to_offset(addr as usize as *const ());

        (addr as u64, self.datamatch)
    }
}

pub fn new_vm_to_host_door_bell(
    vm: &VmFd,
    addr: IoEventAddress,
    datamatch: u64,
) -> (VMToHostDoorBell, VMToHostDoorBellWaiter) {
    let doorbellwaiter = VMToHostDoorBellWaiter::new(vm, &addr, datamatch);
    let doorbell = VMToHostDoorBell::new(addr, datamatch);
    (doorbell, doorbellwaiter)
}
