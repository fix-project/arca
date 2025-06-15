use core::{
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::BuddyAllocator;

extern crate alloc;
use alloc::boxed::Box;

pub struct RefCnt<T: ?Sized> {
    ptr: *mut T,
}

unsafe impl<T: Sync + Send + ?Sized> Send for RefCnt<T> {}
unsafe impl<T: Sync + Send + ?Sized> Sync for RefCnt<T> {}

impl<T: ?Sized> RefCnt<T> {
    pub fn refcnt(this: &Self) -> &AtomicUsize {
        debug_assert!(!this.ptr.is_null());
        unsafe { &*BuddyAllocator.refcnt(this.ptr) }
    }

    pub fn into_raw(this: Self) -> *mut T {
        let p = this.ptr;
        core::mem::forget(this);
        p
    }

    /// # Safety
    /// This raw pointer must have been previously returned by a call to [into_raw].
    pub unsafe fn from_raw(ptr: *mut T) -> RefCnt<T> {
        debug_assert!(!ptr.is_null());
        let rc = RefCnt { ptr };
        assert_ne!((*BuddyAllocator.refcnt(ptr)).load(Ordering::SeqCst), 0);
        rc
    }

    /// # Safety
    /// This raw pointer and allocator pair must have been previously returned by a call to
    /// [into_raw_with_allocator].
    pub unsafe fn from_raw_in(ptr: *mut T, _: BuddyAllocator) -> RefCnt<T> {
        Self::from_raw(ptr)
    }

    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        let refcnt = Self::refcnt(this);
        if refcnt.load(Ordering::SeqCst) == 1 {
            unsafe { Some(Self::get_mut_unchecked(this)) }
        } else {
            None
        }
    }

    /// # Safety
    ///
    /// Gets a mutable reference to the contents of this RefCnt. The caller is responsible for
    /// ensuring there are no other references to these contents which would violate Rust's
    /// aliasing model.
    pub unsafe fn get_mut_unchecked(this: &mut Self) -> &mut T {
        unsafe { &mut *this.ptr }
    }
}

impl<T: Clone> RefCnt<T> {
    pub fn make_mut(this: &mut Self) -> &mut T {
        let refcnt = Self::refcnt(this);
        if refcnt.load(Ordering::SeqCst) == 1 {
            return unsafe { &mut *this.ptr };
        }
        let mut b = Box::new_uninit_in(BuddyAllocator);
        let inner = unsafe {
            b.write((*this.ptr).clone());
            b.assume_init()
        };
        let mut inner = inner.into();
        core::mem::swap(&mut inner, this);
        assert_eq!(Self::refcnt(this).load(Ordering::SeqCst), 1);
        unsafe { &mut *this.ptr }
    }

    pub fn into_unique(this: Self) -> Box<T, BuddyAllocator> {
        let mut this = this;
        Self::make_mut(&mut this);
        assert_eq!(Self::refcnt(&this).swap(0, Ordering::SeqCst), 1);
        let x = unsafe { Box::from_raw_in(this.ptr, BuddyAllocator) };
        core::mem::forget(this);
        x
    }
}

impl<T: ?Sized> Drop for RefCnt<T> {
    fn drop(&mut self) {
        let refcnt = Self::refcnt(self);
        let decrement = refcnt.fetch_sub(1, Ordering::SeqCst);
        if decrement == 1 {
            unsafe {
                let _ = Box::from_raw_in(self.ptr, BuddyAllocator);
                assert_eq!(Self::refcnt(self).swap(0, Ordering::SeqCst), 0);
            }
        }
    }
}

impl<T: ?Sized> Clone for RefCnt<T> {
    fn clone(&self) -> Self {
        Self::refcnt(self).fetch_add(1, Ordering::SeqCst);
        Self { ptr: self.ptr }
    }
}

impl<T: ?Sized> Deref for RefCnt<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: ?Sized> From<Box<T, BuddyAllocator>> for RefCnt<T> {
    fn from(value: Box<T, BuddyAllocator>) -> Self {
        let ptr = Box::into_raw(value);
        let rc = RefCnt { ptr };
        assert_eq!(Self::refcnt(&rc).swap(1, Ordering::SeqCst), 0);
        rc
    }
}

impl<T: ?Sized + core::fmt::Debug> core::fmt::Debug for RefCnt<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized + PartialEq> PartialEq for RefCnt<T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.ptr, other.ptr) || (*self == *other)
    }
}

impl<T: ?Sized + Eq> Eq for RefCnt<T> {}

#[cfg(test)]
mod tests {
    extern crate test;

    use super::*;
    use test::Bencher;

    #[test]
    fn test_from_box() {
        let x = Box::new_in(10, BuddyAllocator);
        core::mem::drop(x);
    }

    #[test]
    fn test_clone_drop() {
        let x = Box::new_in(10, BuddyAllocator);
        let y: RefCnt<i32> = x.into();
        let z = y.clone();
        core::mem::drop(y);
        core::mem::drop(z);
    }

    #[test]
    fn test_make_mut() {
        let x = Box::new_in(10, BuddyAllocator);
        let mut y: RefCnt<i32> = x.into();
        let q = RefCnt::make_mut(&mut y);
        let _ = q;

        let z = y.clone();
        let q = RefCnt::make_mut(&mut y);
        *q = 31;
        assert_eq!(*z, 10);
        assert_eq!(*y, 31);

        core::mem::drop(y);

        core::mem::drop(z);
    }

    #[bench]
    fn bench_clone_drop(b: &mut Bencher) {
        let x = Box::new_in(10, BuddyAllocator);

        let x: RefCnt<i32> = x.into();
        b.iter(|| {
            let _ = x.clone();
        });
        core::mem::drop(x);
    }

    #[bench]
    fn bench_make_mut(b: &mut Bencher) {
        let x = Box::new_in(10, BuddyAllocator);
        let x: RefCnt<i32> = x.into();
        b.iter(|| {
            let mut y = x.clone();
            *(RefCnt::make_mut(&mut y)) = 1;
        });
        assert_eq!(*x, 10);
        core::mem::drop(x);
    }
}
