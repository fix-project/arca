use std::{
    alloc::{Allocator, Global, Layout},
    ffi::CStr,
    fs::File,
    os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd},
    ptr::NonNull,
};

use common::{
    vhost::{VSockMetadata, VirtQueueMetadata},
    BuddyAllocator,
};

use anyhow::{anyhow, Result};
use vmm_sys_util::eventfd::EventFd;

mod bindings {
    #![allow(non_upper_case_globals)]
    #![allow(non_camel_case_types)]
    #![allow(unused)]

    include!(concat!(env!("OUT_DIR"), "/vhost_bindings.rs"));
}

macro_rules! check_syscall {
    ($e: expr) => {{
        let result = $e;
        if result == -1 {
            let errno = libc::__errno_location().read();
            let error = CStr::from_ptr(libc::strerror(errno));
            log::error!("system call returned {errno}");
            Err(anyhow::anyhow!(
                "system call {} failed: {}",
                stringify!($e),
                error.display()
            ))
        } else {
            Ok(())
        }
    }};
}

pub struct VSockBackend {
    fd: OwnedFd,
    cid: u64,
    rx: VirtQueue,
    tx: VirtQueue,
    _kick: EventFd,
    _call: EventFd,
}

impl VSockBackend {
    pub fn new(cid: u64, descriptors: usize, kick: EventFd, call: EventFd) -> Result<Self> {
        let file = File::options()
            .read(true)
            .write(true)
            .open("/dev/vhost-vsock")
            .expect("could not open /dev/vhost-vsock");
        let owned = unsafe {
            let raw = file.into_raw_fd();
            OwnedFd::from_raw_fd(raw)
        };

        unsafe {
            check_syscall!(libc::ioctl(
                owned.as_raw_fd(),
                bindings::VHOST_SET_OWNER as u64,
            ))?;

            check_syscall!(libc::ioctl(
                owned.as_raw_fd(),
                bindings::VHOST_VSOCK_SET_GUEST_CID as u64,
                &cid,
            ))?;

            let mut features: u64 = 0;
            check_syscall!(libc::ioctl(
                owned.as_raw_fd(),
                bindings::VHOST_GET_FEATURES as u64,
                &mut features
            ))?;

            if features & (1 << 1) == 0 {
                return Err(anyhow!("This host does not support seqpacket vsock."));
            }
            if features & (1 << 2) != 0 {
                return Err(anyhow!("This host does not support stream vsock."));
            }

            features = (1 << 1) // supports seqpacket
            | (1 << 28) // supports indirect descriptors
            | (1 << 32);

            check_syscall!(libc::ioctl(
                owned.as_raw_fd(),
                bindings::VHOST_SET_FEATURES as u64,
                &features
            ))?;

            let layout = Layout::new::<bindings::vhost_memory>()
                .extend(Layout::new::<bindings::vhost_memory_region>())
                .unwrap()
                .0;
            let ptr = Global.allocate_zeroed(layout).unwrap();

            let ptr: *mut bindings::vhost_memory =
                std::mem::transmute(&raw mut (*NonNull::as_ptr(ptr))[0]);
            let mut memory = Box::from_raw(ptr);
            memory.nregions = 1;
            let regions = memory.regions.as_mut_slice(1);
            regions[0] = bindings::vhost_memory_region {
                guest_phys_addr: 0,
                memory_size: BuddyAllocator.len() as u64,
                userspace_addr: BuddyAllocator.base() as u64,
                flags_padding: 0,
            };

            check_syscall!(libc::ioctl(
                owned.as_raw_fd(),
                bindings::VHOST_SET_MEM_TABLE as u64,
                memory
            ))?;
        }

        let mut rx = VirtQueue::new(descriptors);
        let mut tx = VirtQueue::new(descriptors);

        rx.attach(owned.as_fd(), VirtQueueType::Receive, &kick, &call)?;
        tx.attach(owned.as_fd(), VirtQueueType::Transmit, &kick, &call)?;

        Ok(Self {
            fd: owned,
            cid,
            rx,
            tx,
            _kick: kick,
            _call: call,
        })
    }

    pub fn cid(&self) -> u64 {
        self.cid
    }

    pub fn set_running(&mut self, running: bool) -> Result<()> {
        unsafe {
            let running: std::ffi::c_int = if running { 1 } else { 0 };
            check_syscall!(libc::ioctl(
                self.fd.as_raw_fd(),
                bindings::VHOST_VSOCK_SET_RUNNING as u64,
                &running
            ))?;
        }
        Ok(())
    }

    pub fn metadata(&self) -> VSockMetadata {
        VSockMetadata {
            rx: self.rx.metadata(),
            tx: self.tx.metadata(),
        }
    }
}

impl Drop for VSockBackend {
    fn drop(&mut self) {
        self.set_running(false).unwrap();
    }
}

#[repr(u32)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum VirtQueueType {
    Receive = 0,
    Transmit = 1,
}

#[derive(Clone, Debug)]
struct VirtQueue {
    pub descriptors: usize,
    pub desc: Box<[u8], BuddyAllocator>,
    pub used: Box<[u8], BuddyAllocator>,
    pub avail: Box<[u8], BuddyAllocator>,
}

impl VirtQueue {
    pub fn new(descriptors: usize) -> Self {
        let new_buffer = |size: usize| {
            let slice = Box::new_zeroed_slice_in(2 * size, BuddyAllocator);
            unsafe { slice.assume_init() }
        };
        let desc = new_buffer(descriptors * 16);
        let used = new_buffer(descriptors * 8 + 6);
        let avail = new_buffer(descriptors * 2 + 6);

        VirtQueue {
            descriptors,
            desc,
            used,
            avail,
        }
    }

    pub fn metadata(&self) -> VirtQueueMetadata {
        let desc = (&raw const *self.desc).to_raw_parts();
        let used = (&raw const *self.used).to_raw_parts();
        let avail = (&raw const *self.avail).to_raw_parts();

        let desc = BuddyAllocator.to_offset(desc.0);
        let used = BuddyAllocator.to_offset(used.0);
        let avail = BuddyAllocator.to_offset(avail.0);

        VirtQueueMetadata {
            descriptors: self.descriptors,
            desc,
            used,
            avail,
        }
    }

    pub fn attach(
        &mut self,
        fd: BorrowedFd,
        qtype: VirtQueueType,
        kick: &EventFd,
        call: &EventFd,
    ) -> Result<()> {
        unsafe {
            let vring_addr = bindings::vhost_vring_addr {
                index: qtype as u32,
                flags: 0,
                desc_user_addr: self.desc.as_ptr() as u64,
                used_user_addr: self.used.as_ptr() as u64,
                avail_user_addr: self.avail.as_ptr() as u64,
                log_guest_addr: 0, // logging is disabled
            };

            check_syscall!(libc::ioctl(
                fd.as_raw_fd(),
                bindings::VHOST_SET_VRING_ADDR as u64,
                &vring_addr
            ))?;

            let vring_num = bindings::vhost_vring_state {
                index: qtype as u32,
                num: self.descriptors as u32,
            };
            check_syscall!(libc::ioctl(
                fd.as_raw_fd(),
                bindings::VHOST_SET_VRING_NUM as u64,
                &vring_num
            ))?;

            let vring_file = bindings::vhost_vring_file {
                index: qtype as u32,
                fd: kick.as_raw_fd(),
            };
            check_syscall!(libc::ioctl(
                fd.as_raw_fd(),
                bindings::VHOST_SET_VRING_KICK as u64,
                &vring_file
            ))?;

            let vring_file = bindings::vhost_vring_file {
                index: qtype as u32,
                fd: call.as_raw_fd(),
            };
            check_syscall!(libc::ioctl(
                fd.as_raw_fd(),
                bindings::VHOST_SET_VRING_CALL as u64,
                &vring_file
            ))?;
        }
        Ok(())
    }
}
