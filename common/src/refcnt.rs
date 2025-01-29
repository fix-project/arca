use core::{
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::BuddyAllocator;

extern crate alloc;
use alloc::boxed::Box;

pub struct RefCnt<'a, T: ?Sized> {
    ptr: *mut T,
    allocator: &'a BuddyAllocator<'a>,
}

unsafe impl<T: Sync + Send + ?Sized> Send for RefCnt<'_, T> {}
unsafe impl<T: Sync + Send + ?Sized> Sync for RefCnt<'_, T> {}

impl<'a, T: ?Sized> RefCnt<'a, T> {
    pub fn refcnt(this: &Self) -> &AtomicUsize {
        unsafe { &*this.allocator.refcnt(this.ptr) }
    }

    pub fn into_raw(this: Self) -> *mut T {
        let p = this.ptr;
        core::mem::forget(this);
        p
    }

    pub fn into_raw_with_allocator(this: Self) -> (*mut T, &'a BuddyAllocator<'a>) {
        let allocator = this.allocator;
        (Self::into_raw(this), allocator)
    }

    /// # Safety
    /// This raw pointer and allocator pair must have been previously returned by a call to
    /// [into_raw_with_allocator].
    pub unsafe fn from_raw_in(ptr: *mut T, allocator: &'a BuddyAllocator<'a>) -> RefCnt<'a, T> {
        RefCnt { ptr, allocator }
    }
}

impl<'a, T: Clone> RefCnt<'a, T> {
    pub fn make_mut(this: &mut Self) -> &mut T {
        let refcnt = Self::refcnt(this);
        if refcnt.load(Ordering::SeqCst) == 1 {
            return unsafe { &mut *this.ptr };
        }
        let mut b = Box::new_uninit_in(this.allocator);
        let inner = unsafe {
            b.write((*this.ptr).clone());
            b.assume_init()
        };
        if refcnt.fetch_sub(1, Ordering::SeqCst) == 1 {
            refcnt.store(1, Ordering::SeqCst);
            return unsafe { &mut *this.ptr };
        }
        this.ptr = Box::into_raw(inner);
        Self::refcnt(this).store(1, Ordering::SeqCst);
        unsafe { &mut *this.ptr }
    }

    pub fn into_unique(this: Self) -> Box<T, &'a BuddyAllocator<'a>> {
        let mut this = this;
        Self::make_mut(&mut this);
        Self::refcnt(&this).store(0, Ordering::SeqCst);
        let x = unsafe { Box::from_raw_in(this.ptr, this.allocator) };
        core::mem::forget(this);
        x
    }
}

impl<T: ?Sized> Drop for RefCnt<'_, T> {
    fn drop(&mut self) {
        let refcnt = Self::refcnt(self);
        if refcnt.fetch_sub(1, Ordering::SeqCst) == 1 {
            unsafe {
                let _ = Box::from_raw_in(self.ptr, self.allocator);
            }
        }
    }
}

impl<T: ?Sized> Clone for RefCnt<'_, T> {
    fn clone(&self) -> Self {
        Self::refcnt(self).fetch_add(1, Ordering::SeqCst);
        Self {
            ptr: self.ptr,
            allocator: self.allocator,
        }
    }
}

impl<T: ?Sized> Deref for RefCnt<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<'a, T: ?Sized> From<Box<T, &'a BuddyAllocator<'a>>> for RefCnt<'a, T> {
    fn from(value: Box<T, &'a BuddyAllocator<'a>>) -> Self {
        let allocator = &**Box::allocator(&value);
        let ptr = Box::into_raw(value);
        let rc = RefCnt { ptr, allocator };
        Self::refcnt(&rc).store(1, Ordering::SeqCst);
        rc
    }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for RefCnt<'_, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + PartialEq> PartialEq for RefCnt<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.allocator as *const _, other.allocator as *const _)
            && (core::ptr::eq(self.ptr, other.ptr) || (*self == *other))
    }
}

impl<T: ?Sized + Eq> Eq for RefCnt<'_, T> {}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::Bencher;

    #[test]
    fn test_from_box() {
        let mut region = unsafe { Box::new_uninit_slice(0x100000).assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let original = allocator.used_size();
        let x = Box::new_in(10, &allocator);
        assert_eq!(
            allocator.used_size() - original,
            BuddyAllocator::MIN_ALLOCATION
        );
        core::mem::drop(x);
        assert_eq!(allocator.used_size() - original, 0);
    }

    #[test]
    fn test_clone_drop() {
        let mut region = unsafe { Box::new_uninit_slice(0x100000).assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let original = allocator.used_size();
        let x = Box::new_in(10, &allocator);
        assert_eq!(
            allocator.used_size() - original,
            BuddyAllocator::MIN_ALLOCATION
        );
        let y: RefCnt<i32> = x.into();
        assert_eq!(
            allocator.used_size() - original,
            BuddyAllocator::MIN_ALLOCATION
        );
        let z = y.clone();
        core::mem::drop(y);
        assert_eq!(
            allocator.used_size() - original,
            BuddyAllocator::MIN_ALLOCATION
        );
        core::mem::drop(z);
        assert_eq!(allocator.used_size() - original, 0);
    }

    #[test]
    fn test_make_mut() {
        let mut region = unsafe { Box::new_uninit_slice(0x100000).assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let original = allocator.used_size();
        let check_allocations = |x: usize| {
            assert_eq!(
                allocator.used_size() - original,
                x * BuddyAllocator::MIN_ALLOCATION
            );
        };

        let x = Box::new_in(10, &allocator);
        check_allocations(1);

        let mut y: RefCnt<i32> = x.into();
        check_allocations(1);

        let q = RefCnt::make_mut(&mut y);
        check_allocations(1);
        let _ = q;

        let z = y.clone();
        let q = RefCnt::make_mut(&mut y);
        *q = 31;
        assert_eq!(*z, 10);
        assert_eq!(*y, 31);
        check_allocations(2);

        core::mem::drop(y);
        check_allocations(1);

        core::mem::drop(z);
        check_allocations(0);
    }

    #[bench]
    fn bench_clone_drop(b: &mut Bencher) {
        let mut region = unsafe { Box::new_uninit_slice(0x100000).assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let original = allocator.used_size();
        let x = Box::new_in(10, &allocator);

        let check_allocations = |x: usize| {
            assert_eq!(
                allocator.used_size() - original,
                x * BuddyAllocator::MIN_ALLOCATION
            );
        };
        check_allocations(1);

        let x: RefCnt<i32> = x.into();
        b.iter(|| {
            let _ = x.clone();
        });
        core::mem::drop(x);
        check_allocations(0);
    }

    #[bench]
    fn bench_make_mut(b: &mut Bencher) {
        let mut region = unsafe { Box::new_uninit_slice(0x100000).assume_init() };
        let allocator = BuddyAllocator::new(&mut *region);
        let original = allocator.used_size();
        let x = Box::new_in(10, &allocator);
        let check_allocations = |x: usize| {
            assert_eq!(
                allocator.used_size() - original,
                x * BuddyAllocator::MIN_ALLOCATION
            );
        };
        check_allocations(1);
        let x: RefCnt<i32> = x.into();
        b.iter(|| {
            let mut y = x.clone();
            *(RefCnt::make_mut(&mut y)) = 1;
        });
        assert_eq!(*x, 10);
        core::mem::drop(x);
        check_allocations(0);
    }
}
