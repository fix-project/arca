use crate::prelude::*;

mod avail;
mod desc;
mod idx;
mod used;
mod vring;

use avail::*;
use common::{util::sorter::Sorter, vhost::VirtQueueMetadata};
use desc::*;
use idx::*;
use used::*;
use vring::*;

#[derive(Debug)]
pub struct VirtQueue {
    name: &'static str,
    response_sorter: Sorter<DescriptorIndex, usize>,
    desc: SpinLock<DescTable>,
    used: SpinLock<UsedRing>,
    avail: SpinLock<AvailRing>,
}

impl VirtQueue {
    pub unsafe fn new(name: &'static str, info: VirtQueueMetadata) -> Self {
        let desc = core::ptr::from_raw_parts_mut(vm::pa2ka(info.desc) as *mut (), info.descriptors);
        let used = core::ptr::from_raw_parts_mut(vm::pa2ka(info.used) as *mut (), info.descriptors);
        let avail =
            core::ptr::from_raw_parts_mut(vm::pa2ka(info.avail) as *mut (), info.descriptors);
        VirtQueue {
            name,
            response_sorter: Sorter::new(),
            desc: SpinLock::new(DescTable::new(desc)),
            used: SpinLock::new(UsedRing::new(used)),
            avail: SpinLock::new(AvailRing::new(avail)),
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
        self.car.len()
            + match self.cdr {
                Some(x) => x.len(),
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
}

impl VirtQueue {
    fn load(desc: &mut DescTable, bufs: &BufferChain<'_>) -> DescriptorIndex {
        // TODO: handle descriptor unavailability
        let current = desc.try_allocate().expect("no descriptors available");
        let rest = bufs.cdr.map(|x| Self::load(desc, x));
        desc.get_mut(current).modify(|d| {
            let (p, w) = match &bufs.car {
                Buffer::Immutable(x) => (*x as *const [u8], false),
                Buffer::Mutable(x) => (*x as *const [u8], true),
            };
            let (addr, len) = p.to_raw_parts();
            d.addr = addr as *mut ();
            d.len = len;
            d.next = rest;
            d.device_writeable = w;
        });
        current
    }

    pub async fn send(&self, bufs: &BufferChain<'_>) -> usize {
        unsafe {
            let head = Self::load(&mut self.desc.lock(), bufs);
            let mut rx = self.response_sorter.receiver(head);
            self.avail.lock().push(head);
            let result = rx.recv().await.unwrap();
            self.response_sorter.clear(head);
            let _ = rx;
            self.desc.lock().liberate(head);
            result
        }
    }

    pub fn try_poll(&self) -> Option<()> {
        let mut used = self.used.try_lock()?;
        while let Some(used) = used.pop() {
            let x = self
                .response_sorter
                .sender()
                .send_blocking(used.id().into(), used.len() as usize);
            if x.is_err() {
                panic!(
                    "{} had error while waking up descriptor {used:?}: {x:?}",
                    self.name
                );
            }
        }
        Some(())
    }
}
