use crate::{io, prelude::*};

mod desc;
mod idx;
mod vring;

use common::{util::sorter::Sorter, vhost::VirtQueueMetadata};
use desc::*;
use idx::*;
use vring::*;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct UsedElement {
    id: u32,
    len: u32,
}

#[derive(Debug)]
pub struct VirtQueue {
    name: &'static str,
    response_sorter: Sorter<DescriptorIndex, usize>,
    desc: Mutex<DescTable>,
    used: Mutex<DeviceRing<UsedElement>>,
    avail: Mutex<DriverRing<DescriptorIndex>>,
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
        VirtQueue {
            name,
            response_sorter: Sorter::new(),
            desc: Mutex::new(DescTable::new(name, desc)),
            used: Mutex::new(DeviceRing::new(name, used)),
            avail: Mutex::new(DriverRing::new(name, avail)),
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
        let mut buf = Box::new_uninit_slice(bufs.len());
        let descs = loop {
            let descs = {
                let mut desc = self.desc.lock().await;
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
        let mut desc = self.desc.lock().await;
        while let Some(x) = current {
            let idx = descs[i];
            desc.get_mut(idx).modify(|d| {
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
                desc.get_mut(previous).modify(|d| {
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

    pub async fn send(&self, bufs: &BufferChain<'_>) -> usize {
        unsafe {
            let head = self.load(bufs).await;
            let rx = self.response_sorter.receiver(head);
            self.avail.lock().await.send(head);
            if !self.used.lock().await.avail_notifications_suppressed() {
                io::outl(0xf4, 0);
            }
            let result = rx.recv().await;
            let result = match result {
                Ok(result) => result,
                Err(e) => {
                    panic!("error while waiting for {head:?}: {e:?}");
                }
            };
            core::mem::drop(rx);
            self.desc.lock().await.liberate(head);
            result
        }
    }

    pub fn poll(&self) {
        let mut used = self.used.spin_lock();
        unsafe {
            while let Some(used) = used.recv() {
                let x = self
                    .response_sorter
                    .sender()
                    .send_blocking(used.id.into(), used.len as usize);
                if x.is_err() {
                    panic!(
                        "{} had error while waking up descriptor {used:?}: {x:?}",
                        self.name
                    );
                }
            }
        }
    }
}
