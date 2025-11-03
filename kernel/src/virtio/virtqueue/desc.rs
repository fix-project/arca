use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

use super::*;

#[derive(Debug)]
pub struct DescTable {
    name: &'static str,
    free_index: Option<DescriptorIndex>,
    free_descs: AtomicUsize,
    table: *mut [Desc],
}

unsafe impl Send for DescTable {}

impl DescTable {
    pub unsafe fn new(name: &'static str, p: *mut [Desc]) -> Self {
        let mut x = Self {
            name,
            free_index: None,
            free_descs: AtomicUsize::new(0),
            table: p,
        };
        for i in 0..p.len() {
            x.liberate(i.into());
        }
        x
    }

    pub fn get_mut(&mut self, index: DescriptorIndex) -> &mut Desc {
        unsafe { &mut (*self.table)[usize::from(index)] }
    }

    pub unsafe fn get_mut_unchecked(&self, index: DescriptorIndex) -> &mut Desc {
        unsafe { &mut (*self.table)[usize::from(index)] }
    }

    pub fn try_allocate(&mut self) -> Option<DescriptorIndex> {
        let idx = self.free_index?;
        // safety: since this was on the free list, the device isn't using it
        self.free_index = self.get_mut(idx).modify(|desc| {
            let next = desc.next;
            desc.next = None;
            next
        });
        log::debug!("{} allocated {idx:?}", self.name);
        self.free_descs.fetch_sub(1, Ordering::SeqCst);
        Some(idx)
    }

    pub fn try_allocate_many<'buf>(
        &mut self,
        buf: &'buf mut [MaybeUninit<DescriptorIndex>],
    ) -> Option<&'buf mut [DescriptorIndex]> {
        if buf.is_empty() {
            return Some(unsafe { buf.assume_init_mut() });
        }
        if self.free_descs.load(Ordering::SeqCst) < buf.len() {
            return None;
        }
        let idx = self.try_allocate()?;
        if self.try_allocate_many(&mut buf[1..]).is_none() {
            self.liberate(idx);
            None
        } else {
            let buf = unsafe { buf.assume_init_mut() };
            buf[0] = idx;
            Some(buf)
        }
    }

    pub fn liberate(&mut self, idx: DescriptorIndex) {
        let head = self.free_index;
        log::debug!("{} liberated {idx:?}", self.name);
        let current = self.get_mut(idx);
        let mut tbd = None;
        current.modify(|current| {
            current.addr = core::ptr::null_mut();
            current.len = 0;
            tbd = current.next;
            current.next = head;
        });
        self.free_index = Some(idx);
        self.free_descs.fetch_add(1, Ordering::SeqCst);
        if let Some(tbd) = tbd {
            self.liberate(tbd);
        }
    }
}

mod inner {
    use super::*;

    #[repr(C)]
    #[derive(Debug, Copy, Clone)]
    pub struct Desc {
        addr: u64,
        len: u32,
        flags: u16,
        next: u16,
    }

    pub struct DescData {
        pub addr: *mut (),
        pub len: usize,
        pub device_writeable: bool,
        pub next: Option<DescriptorIndex>,
    }

    impl Desc {
        pub fn get(&self) -> DescData {
            let Desc {
                addr,
                len,
                flags,
                next,
            } = unsafe { (&raw const *self).read_volatile() };
            DescData {
                addr: vm::pa2ka(addr as usize),
                len: len as usize,
                device_writeable: flags & 2 == 2,
                next: if flags & 1 == 1 {
                    Some(next.into())
                } else {
                    None
                },
            }
        }

        pub fn set(&mut self, value: DescData) {
            let desc = Desc {
                addr: vm::ka2pa(value.addr) as u64,
                len: value.len as u32,
                flags: if value.device_writeable { 2 } else { 0 }
                    | if value.next.is_some() { 1 } else { 0 },
                next: value.next.map(|x| x.get()).unwrap_or(0),
            };
            unsafe { (&raw mut *self).write_volatile(desc) }
        }

        pub fn modify<T>(&mut self, f: impl FnOnce(&mut DescData) -> T) -> T {
            let mut desc = self.get();
            let result = f(&mut desc);
            self.set(desc);
            result
        }
    }
}
use inner::*;
