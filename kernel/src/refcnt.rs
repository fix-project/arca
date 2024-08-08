use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::boxed::Box;

use crate::{buddy, page::Page, vm};

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

pub type Page4KB = RcPage<[u8; 1 << 12]>;
pub type Page2MB = RcPage<[u8; 1 << 21]>;
pub type Page1GB = RcPage<[u8; 1 << 30]>;

#[derive(Debug)]
pub struct RcPage<T> {
    ptr: *mut T,
    refcnt: *const AtomicUsize,
}

impl<T> RcPage<T> {
    pub fn new(value: T) -> Self {
        Page::from(value).into()
    }

    pub fn as_ptr(&self) -> *mut T {
        self.ptr
    }

    pub fn into_raw(self) -> *mut T {
        let ptr = self.as_ptr();
        core::mem::forget(self);
        ptr
    }

    /// # Safety
    /// This pointer must have come from [into_raw] and may only be passed to this function once.
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self {
            ptr,
            refcnt: refcnt(ptr),
        }
    }
}

impl<const N: usize> RcPage<[u8; N]> {
    pub fn new_bytes() -> Self {
        Page::new().into()
    }
}

impl<T: Default> Default for RcPage<T> {
    fn default() -> Self {
        Page::<T>::default().into()
    }
}

impl<T> From<Page<T>> for RcPage<T> {
    fn from(value: Page<T>) -> Self {
        let ptr = value.into_raw();
        let refcnt = refcnt(ptr);
        unsafe {
            (*refcnt).store(1, Ordering::SeqCst);
        }
        RcPage { ptr, refcnt }
    }
}

impl<T> Drop for RcPage<T> {
    fn drop(&mut self) {
        unsafe {
            if (*self.refcnt).fetch_sub(1, Ordering::SeqCst) == 1 {
                let block = Page::from_raw(self.ptr);
                core::mem::drop(block);
            }
        }
    }
}

impl<T> Clone for RcPage<T> {
    fn clone(&self) -> Self {
        unsafe {
            (*self.refcnt).fetch_add(1, Ordering::SeqCst);
            Self {
                ptr: self.ptr,
                refcnt: self.refcnt,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_alloc() {
        Page4KB::new_bytes();
    }

    #[bench]
    pub fn bench_alloc_4kb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let x = Page4KB::new_bytes();
            core::mem::forget(x);
        });
    }

    #[bench]
    pub fn bench_clone_4kb(bench: impl FnOnce(&dyn Fn())) {
        let x = Page4KB::new_bytes();
        bench(&|| {
            let y = x.clone();
            core::mem::forget(y);
        });
    }

    #[bench]
    pub fn bench_alloc_free_4kb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = Page4KB::new_bytes();
        });
    }

    #[bench]
    pub fn bench_alloc_free_2mb(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            let _ = Page2MB::new_bytes();
        });
    }
}
