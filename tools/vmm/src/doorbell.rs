use common::{buddy::ioaddr, pipe::DoorBell, protocol::control::VMToHostDoorBellData};
use kvm_ioctls::{IoEventAddress, VmFd};
use vmm_sys_util::eventfd::{EFD_NONBLOCK, EventFd};

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
        loop {
            match self.fd.write(1) {
                Ok(()) => return,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                // A saturated eventfd already carries a pending notification.
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return,
                Err(e) => panic!("Failed to ring I/O doorbell: {e}"),
            }
        }
    }
}

#[derive(Debug)]
pub struct VMToHostDoorBellWaiter {
    pub fd: EventFd,
}

impl VMToHostDoorBellWaiter {
    pub fn wait(&self) {
        loop {
            match self.fd.read() {
                Ok(_) => return,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => panic!("Failed to wait for I/O doorbell: {e}"),
            }
        }
    }

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
    datamatch: u64,
}

impl VMToHostDoorBell {
    fn new(datamatch: u64) -> Self {
        Self { datamatch }
    }

    pub fn into_raw_parts(self) -> VMToHostDoorBellData {
        VMToHostDoorBellData {
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
    datamatch: u64,
) -> (VMToHostDoorBell, VMToHostDoorBellWaiter) {
    let doorbellwaiter =
        VMToHostDoorBellWaiter::new(vm, &IoEventAddress::Mmio(ioaddr()), datamatch);
    let doorbell = VMToHostDoorBell::new(datamatch);
    (doorbell, doorbellwaiter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipe::{Error, GuestPipe};
    use std::{sync::mpsc, time::Duration};

    // Exercise the real blocking eventfd wait path without requiring KVM.
    fn pipes() -> (GuestPipe, GuestPipe) {
        let p_readable = EventFd::new(0).unwrap();
        let p_writable = EventFd::new(0).unwrap();
        let q_readable = EventFd::new(0).unwrap();
        let q_writable = EventFd::new(0).unwrap();
        let (p, q) = common::pipe::pipe(
            8,
            HostToVMDoorBell {
                fd: q_readable.try_clone().unwrap(),
            },
            HostToVMDoorBell {
                fd: q_writable.try_clone().unwrap(),
            },
            HostToVMDoorBell {
                fd: p_readable.try_clone().unwrap(),
            },
            HostToVMDoorBell {
                fd: p_writable.try_clone().unwrap(),
            },
        );
        (
            GuestPipe::new(
                p,
                VMToHostDoorBellWaiter { fd: p_readable },
                VMToHostDoorBellWaiter { fd: p_writable },
            ),
            GuestPipe::new(
                q,
                VMToHostDoorBellWaiter { fd: q_readable },
                VMToHostDoorBellWaiter { fd: q_writable },
            ),
        )
    }

    #[test]
    fn eventfd_backpressure_and_wraparound() {
        let (mut p, mut q) = pipes();
        let (done, result) = mpsc::channel();
        let peer_done = done.clone();
        let peer = std::thread::spawn(move || {
            let mut bytes = vec![0; 4096];
            q.read_exact(&mut bytes).unwrap();
            q.write_exact(&bytes).unwrap();
            peer_done.send(()).unwrap();
        });
        let sender = std::thread::spawn(move || {
            let bytes: Vec<_> = (0..4096).map(|i| i as u8).collect();
            p.write_exact(&bytes).unwrap();
            let mut response = vec![0; bytes.len()];
            p.read_exact(&mut response).unwrap();
            assert_eq!(response, bytes);
            done.send(()).unwrap();
        });
        result.recv_timeout(Duration::from_secs(5)).unwrap();
        result.recv_timeout(Duration::from_secs(5)).unwrap();
        peer.join().unwrap();
        sender.join().unwrap();
    }

    #[test]
    fn hangup_wakes_blocked_reader_and_writer() {
        for writing in [false, true] {
            let (mut p, q) = pipes();
            assert_eq!(p.read(&mut []), Ok(0));
            if writing {
                p.write_exact(b"1234567").unwrap(); // fill the ring
                assert_eq!(p.write(&[]), Ok(0));
            }
            let (done, result) = mpsc::channel();
            let waiter = std::thread::spawn(move || {
                let outcome = if writing {
                    p.write(b"x")
                } else {
                    p.read(&mut [0])
                };
                done.send(outcome).unwrap();
            });
            std::thread::sleep(Duration::from_millis(20));
            drop(q);
            assert_eq!(
                result.recv_timeout(Duration::from_secs(5)).unwrap(),
                Err(Error::Closed)
            );
            waiter.join().unwrap();
        }
    }
}
