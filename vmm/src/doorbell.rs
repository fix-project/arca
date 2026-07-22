use common::{pipe::DoorBell, protocol::control::VMToHostDoorBellData};
use kvm_ioctls::{IoEventAddress, VmFd};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};

#[derive(Debug)]
pub struct HostToVMDoorBell {
    fd: EventFd,
}

const HOST_TO_VM_GSI: u32 = 2;

impl HostToVMDoorBell {
    pub fn new(vm: &VmFd) -> Self {
        let evtfd = EventFd::new(EFD_NONBLOCK).unwrap();
        vm.register_irqfd(&evtfd, HOST_TO_VM_GSI)
            .expect("Failed to register irqfd");
        Self { fd: evtfd }
    }
}

impl DoorBell for HostToVMDoorBell {
    fn ring(&self) {
        while self.fd.write(1).is_err() {}
    }
}

#[derive(Debug)]
pub struct VMToHostDoorBellWaiter {
    pub fd: EventFd,
}

impl VMToHostDoorBellWaiter {
    /// Each eventfd needs to have a unique {addr, datamatch} pair, and it is
    /// allowed to have multiple eventfds registered at the same address with
    /// different datamatch. The caller needs to guarantee that {addr, datamatch}
    /// hasn't been registered before
    fn new(vm: &VmFd, addr: &IoEventAddress, datamatch: u64) -> Self {
        let evtfd = EventFd::new(0).unwrap();
        vm.register_ioevent(&evtfd, addr, datamatch)
            .expect("Failed to register ioevent");
        Self { fd: evtfd }
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

    pub fn into_raw_parts(self) -> VMToHostDoorBellData {
        let addr = match self.addr {
            IoEventAddress::Pio(_) => todo!(),
            IoEventAddress::Mmio(addr) => addr,
        };

        VMToHostDoorBellData {
            addr,
            datamatch: self.datamatch,
        }
    }
}

impl DoorBell for VMToHostDoorBell {
    fn ring(&self) {
        panic!("Ringing at the wrong location")
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
