use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug)]
pub struct SpinLock<T> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

#[derive(Debug)]
pub struct SpinLockGuard<'a, T> {
    lock: &'a SpinLock<T>,
    data: &'a mut T,
}

impl<T> SpinLock<T> {
    pub const fn new(data: T) -> SpinLock<T> {
        SpinLock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn lock(&self) -> SpinLockGuard<T> {
        while self
            .lock
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            unsafe { core::arch::x86_64::_mm_pause() }
        }
        SpinLockGuard {
            lock: self,
            data: unsafe { &mut *self.data.get() },
        }
    }
}

impl<T> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.lock.store(false, Ordering::SeqCst);
    }
}

unsafe impl<T> Sync for SpinLock<T> {}

impl<T> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
