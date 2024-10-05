use core::{
    mem::MaybeUninit,
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::boxed::Box;

use crate::{buddy, page::UniquePage, vm};

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

fn refcnt<T>(ptr: *const T) -> *const AtomicUsize {
    let addr = vm::ka2pa(ptr);
    unsafe { &REFERENCE_COUNTS.assume_init_ref()[addr / 4096] }
}

pub type Page4KB = [u8; 1 << 12];
pub type Page2MB = [u8; 1 << 21];
pub type Page1GB = [u8; 1 << 30];

pub type SharedPage4KB = SharedPage<[u8; 1 << 12]>;
pub type SharedPage2MB = SharedPage<[u8; 1 << 21]>;
pub type SharedPage1GB = SharedPage<[u8; 1 << 30]>;

#[derive(Debug)]
pub struct SharedPage<T> {
    ptr: *mut T,
}

unsafe impl<T: Send> Send for SharedPage<T> {}
unsafe impl<T: Sync> Sync for SharedPage<T> {}

impl<T> SharedPage<T> {
    pub fn new(value: T) -> Self {
        UniquePage::from(value).into()
    }

    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    pub fn into_raw(self) -> *mut T {
        let ptr = self.as_ptr();
        core::mem::forget(self);
        ptr
    }

    pub fn refcnt(&self) -> *const AtomicUsize {
        refcnt(self.ptr)
    }

    /// # Safety
    /// This pointer must have come from [into_raw] and may only be passed to this function once.
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self { ptr }
    }
}

impl<T: Clone> SharedPage<T> {
    pub fn make_mut(&mut self) -> &mut T {
        unsafe {
            if (*self.refcnt()).load(Ordering::SeqCst) == 1 {
                // only reference; access is safe
                return &mut *self.ptr;
            }
            let copied = UniquePage::from((*self.ptr).clone());
            if (*self.refcnt()).fetch_sub(1, Ordering::SeqCst) == 1 {
                // now only reference; discard clone
                (*self.refcnt()).store(1, Ordering::SeqCst);
                return &mut *self.ptr;
            }
            self.ptr = copied.into_raw();
            (*self.refcnt()).store(1, Ordering::SeqCst);
            &mut *self.ptr
        }
    }
}

impl<T> Deref for SharedPage<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<const N: usize> SharedPage<[u8; N]> {
    pub fn new_bytes() -> Self {
        UniquePage::new().into()
    }
}

impl<T: Default> Default for SharedPage<T> {
    fn default() -> Self {
        UniquePage::<T>::default().into()
    }
}

impl<T> From<UniquePage<T>> for SharedPage<T> {
    fn from(value: UniquePage<T>) -> Self {
        let ptr = value.into_raw();
        let refcnt = refcnt(ptr);
        unsafe {
            (*refcnt).store(1, Ordering::SeqCst);
        }
        SharedPage { ptr }
    }
}

impl<T> Drop for SharedPage<T> {
    fn drop(&mut self) {
        unsafe {
            if (*self.refcnt()).fetch_sub(1, Ordering::SeqCst) == 1 {
                let block = UniquePage::from_raw(self.ptr);
                core::mem::drop(block);
            }
        }
    }
}

impl<T> Clone for SharedPage<T> {
    fn clone(&self) -> Self {
        unsafe {
            (*self.refcnt()).fetch_add(1, Ordering::SeqCst);
            Self { ptr: self.ptr }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_alloc() {
        SharedPage4KB::new_bytes();
    }

    #[bench]
    pub fn bench_alloc_4kb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let x = SharedPage4KB::new_bytes();
            core::mem::forget(x);
        });
    }

    #[bench]
    pub fn bench_clone_4kb(bench: impl FnOnce(&dyn Fn())) {
        let x = SharedPage4KB::new_bytes();
        bench(&|| {
            let y = x.clone();
            core::mem::forget(y);
        });
    }

    #[bench]
    pub fn bench_alloc_free_4kb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = SharedPage4KB::new_bytes();
        });
    }

    #[bench]
    pub fn bench_alloc_free_2mb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = SharedPage2MB::new_bytes();
        });
    }
}
