use core::{
    future::Future,
    task::{Poll, Waker},
};

use crate::{io, prelude::*};
use common::util::rwlock::RwLock;

mod desc;
mod idx;
mod vring;

use common::vhost::VirtQueueMetadata;
use desc::*;
use idx::*;
use vring::*;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct UsedElement {
    id: u32,
    len: u32,
}

#[derive(Default)]
pub struct NotificationChannel {
    waker: Option<Waker>,
    data: Option<usize>,
}

pub struct Notification {
    channel: Arc<SpinLock<NotificationChannel>>,
}

impl Future for Notification {
    type Output = usize;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> Poll<Self::Output> {
        let mut channel = self.channel.lock();
        if let Some(value) = channel.data.take() {
            Poll::Ready(value)
        } else {
            channel.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct VirtQueue {
    _name: &'static str,
    notifications: Box<[Arc<SpinLock<NotificationChannel>>]>,
    desc: RwLock<DescTable>,
    used: RwLock<DeviceRing<UsedElement>>,
    avail: RwLock<DriverRing<DescriptorIndex>>,
}

impl VirtQueue {
    /// # Safety
    ///
    /// `info` must describe a valid VirtQueue with attached device, where the negotiated features
    /// match those available in this driver. The VirtQueue must not be attached to any other
    /// driver.
    pub unsafe fn new(name: &'static str, info: VirtQueueMetadata) -> Self {
        let desc = core::ptr::from_raw_parts_mut(vm::pa2ka::<()>(info.desc), info.descriptors);
        let used = core::ptr::from_raw_parts_mut(vm::pa2ka::<()>(info.used), info.descriptors);
        let avail = core::ptr::from_raw_parts_mut(vm::pa2ka::<()>(info.avail), info.descriptors);
        let mut notifications = vec![];
        notifications.resize_with(info.descriptors, Default::default);
        VirtQueue {
            _name: name,
            notifications: notifications.into(),
            desc: RwLock::new(DescTable::new(name, desc)),
            used: RwLock::new(DeviceRing::new(name, used)),
            avail: RwLock::new(DriverRing::new(name, avail)),
        }
    }
}

#[derive(Debug)]
pub struct BufferChain<'a> {
    car: Buffer<'a>,
    cdr: Option<&'a BufferChain<'a>>,
}

impl<'a> BufferChain<'a> {
    pub fn new(x: &'a [u8]) -> Self {
        BufferChain {
            car: Buffer::Immutable(x),
            cdr: None,
        }
    }

    pub fn new_mut(x: &'a mut [u8]) -> Self {
        BufferChain {
            car: Buffer::Mutable(x),
            cdr: None,
        }
    }

    pub fn cons<'b>(x: &'a [u8], other: Option<&'b BufferChain<'b>>) -> Self
    where
        'b: 'a,
    {
        BufferChain {
            car: Buffer::Immutable(x),
            cdr: other,
        }
    }

    pub fn cons_mut<'b>(x: &'a mut [u8], other: Option<&'b BufferChain<'b>>) -> Self
    where
        'b: 'a,
    {
        BufferChain {
            car: Buffer::Mutable(x),
            cdr: other,
        }
    }

    pub fn len(&self) -> usize {
        1 + match self.cdr {
            Some(x) => x.len(),
            None => 0,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn size(&self) -> usize {
        self.car.len()
            + match self.cdr {
                Some(x) => x.size(),
                None => 0,
            }
    }
}

#[derive(Debug)]
pub enum Buffer<'a> {
    Immutable(&'a [u8]),
    Mutable(&'a mut [u8]),
}

impl Buffer<'_> {
    pub fn len(&self) -> usize {
        match self {
            Buffer::Immutable(items) => items.len(),
            Buffer::Mutable(items) => items.len(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl VirtQueue {
    async fn load(&self, bufs: &BufferChain<'_>) -> DescriptorIndex {
        unsafe {
            let mut buf = Box::new_uninit_slice(bufs.len());
            let descs = loop {
                let descs = {
                    let mut desc = self.desc.lock();
                    desc.try_allocate_many(&mut buf)
                };
                if let Some(descs) = descs {
                    break descs;
                }
                // TODO: handle descriptor unavailability better
                log::warn!("out of descriptors");
                crate::rt::wfi().await;
            };
            let mut head = None;
            let mut previous = None;
            let mut current = Some(bufs);
            let mut i = 0;
            while let Some(x) = current {
                let idx = descs[i];
                let desc = self.desc.read();
                desc.get_mut_unchecked(idx).modify(|d| {
                    let (p, w) = match &x.car {
                        Buffer::Immutable(x) => (*x as *const [u8], false),
                        Buffer::Mutable(x) => (*x as *const [u8], true),
                    };
                    let (addr, len) = p.to_raw_parts();
                    d.addr = addr as *mut ();
                    d.len = len;
                    d.next = None;
                    d.device_writeable = w;
                });
                if let Some(previous) = previous {
                    desc.get_mut_unchecked(previous).modify(|d| {
                        d.next = Some(idx);
                    });
                }
                previous = Some(idx);

                if head.is_none() {
                    head = Some(idx);
                }
                current = x.cdr;
                i += 1;
            }
            head.unwrap()
        }
    }

    async unsafe fn notification(&self, head: DescriptorIndex) -> usize {
        Notification {
            channel: self.notifications[head.get() as usize].clone(),
        }
        .await
    }

    async unsafe fn mark_avail(&self, head: DescriptorIndex) {
        let mut avail = self.avail.lock();
        avail.send(head);
    }

    async unsafe fn mark_free(&self, head: DescriptorIndex) {
        let mut desc = self.desc.lock();
        desc.liberate(head);
    }

    async unsafe fn send_interrupt(&self) {
        if !self.used.read().avail_notifications_suppressed() {
            io::outl(0xf4, 0);
        }
    }

    pub async fn send(&self, bufs: &BufferChain<'_>) -> usize {
        unsafe {
            let head = self.load(bufs).await;
            self.mark_avail(head).await;
            self.send_interrupt().await;
            let result = self.notification(head).await;
            self.mark_free(head).await;
            result
        }
    }

    pub fn poll(&self) {
        let Some(mut used) = self.used.try_lock() else {
            return;
        };
        unsafe {
            while let Some(used) = used.recv() {
                let mut notification = self.notifications[used.id as usize].lock();
                notification.data = Some(used.len as usize);
                if let Some(waker) = notification.waker.take() {
                    core::mem::drop(notification);
                    waker.wake();
                }
            }
        }
    }
}
