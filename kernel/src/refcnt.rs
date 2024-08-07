use core::{
    mem::MaybeUninit,
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::boxed::Box;

use crate::{buddy, vm};

pub static mut REFERENCE_COUNTS: MaybeUninit<&'static [AtomicUsize]> = MaybeUninit::uninit();

pub(crate) unsafe fn init() {
    let size = {
        let buddy = buddy::PHYSICAL_ALLOCATOR.lock();
        let buddy = buddy
            .get()
            .expect("attempted to initialize reference-counting allocator before buddy allocator");
        buddy.address_space_size() / buddy::BuddyAllocator::MIN_ALLOCATION
    };
    let array: &'static [AtomicUsize] =
        MaybeUninit::slice_assume_init_ref(Box::leak::<'static>(Box::new_zeroed_slice(size)));
    REFERENCE_COUNTS.write(array);
}

pub type Page4KB = Block<12>;
pub type Page2MB = Block<21>;
pub type Page1GB = Block<30>;

#[derive(Debug)]
pub struct Block<const ORDER: usize> {
    base: *mut u8,
    refcnt: &'static AtomicUsize,
}

impl<const N: usize> Block<N> {
    pub const ORDER: usize = N;
    pub const LENGTH: usize = (1 << N);

    pub fn new() -> Block<N> {
        let uniq = buddy::Block::<N>::new().unwrap();
        let base = uniq.into_raw();
        let addr = vm::ka2pa(base);
        let refcnt = unsafe { &REFERENCE_COUNTS.assume_init_ref()[addr / 4096] };
        assert_eq!(refcnt.load(Ordering::Acquire), 0);
        refcnt.store(1, Ordering::Release);
        Block { base, refcnt }
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.base
    }

    pub fn into_raw(self) -> *mut u8 {
        let p = self.base;
        core::mem::forget(self);
        p
    }

    pub fn reference_count(&self) -> usize {
        self.refcnt.load(Ordering::SeqCst)
    }

    pub fn unique(&self) -> bool {
        self.reference_count() == 1
    }

    /// # Safety
    /// This pointer must correspond to the beginning of a valid block with the specified size
    /// (e.g., one created using ::into_raw).  This must only be called once per call to ::into_raw
    /// in order to preserve the correct reference count.
    pub unsafe fn from_raw(raw: *mut u8) -> Block<N> {
        let addr = vm::ka2pa(raw);
        let refcnt = unsafe { &REFERENCE_COUNTS.assume_init_ref()[addr / 4096] };
        Block { base: raw, refcnt }
    }
}

impl<const N: usize> Default for Block<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Drop for Block<N> {
    fn drop(&mut self) {
        if self.unique() {
            self.refcnt.store(0, Ordering::SeqCst);
            let block = unsafe { buddy::Block::<N>::from_raw(self.base) };
            core::mem::drop(block);
        }
    }
}

impl<const N: usize> Deref for Block<N> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.base, 1 << N) }
    }
}

impl<const N: usize> Clone for Block<N> {
    fn clone(&self) -> Self {
        self.refcnt.fetch_add(1, Ordering::SeqCst);
        Self {
            base: self.base,
            refcnt: self.refcnt,
        }
    }
}

impl<const N: usize> From<buddy::Block<N>> for Block<N> {
    fn from(value: buddy::Block<N>) -> Self {
        let base = value.into_raw();
        let addr = vm::ka2pa(base);
        let refcnt = unsafe { &REFERENCE_COUNTS.assume_init_ref()[addr / 4096] };
        assert_eq!(refcnt.load(Ordering::Acquire), 0);
        refcnt.store(1, Ordering::Release);
        Block { base, refcnt }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_alloc() {
        Page4KB::new();
    }

    #[bench]
    pub fn bench_alloc_4kb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let x = Page4KB::new();
            core::mem::forget(x);
        });
    }

    #[bench]
    pub fn bench_clone_4kb(bench: impl FnOnce(&dyn Fn())) {
        let x = Page4KB::new();
        bench(&|| {
            let y = x.clone();
            core::mem::forget(y);
        });
    }

    #[bench]
    pub fn bench_alloc_free_4kb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = Page4KB::new();
        });
    }

    #[bench]
    pub fn bench_alloc_free_2mb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = Page2MB::new();
        });
    }
}
