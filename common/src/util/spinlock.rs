use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
pub struct SpinLock<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

#[derive(Debug)]
pub struct SpinLockGuard<'a, T: ?Sized> {
    lock: &'a SpinLock<T>,
    data: &'a mut T,
}

impl<T: ?Sized> SpinLock<T> {
    pub const fn new(data: T) -> SpinLock<T>
    where
        T: Sized,
    {
        SpinLock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    pub fn try_lock(&self) -> Option<SpinLockGuard<'_, T>> {
        if self
            .lock
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(SpinLockGuard {
                lock: self,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }

    pub fn lock(&self) -> SpinLockGuard<'_, T> {
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }
            core::hint::spin_loop();
        }
    }

    pub fn get(&self) -> *mut T {
        self.data.get()
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.lock();
        f(&mut guard)
    }

    pub fn unlock(guard: SpinLockGuard<'_, T>) {
        SpinLockGuard::unlock(guard);
    }
}

impl<T: ?Sized> SpinLockGuard<'_, T> {
    pub fn unlock(_: Self) {}
}

impl<T: ?Sized> Drop for SpinLockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.lock.store(false, Ordering::Release);
    }
}

unsafe impl<T: Send + ?Sized> Send for SpinLock<T> {}
unsafe impl<T: Send + ?Sized> Sync for SpinLock<T> {}

impl<T: ?Sized> Deref for SpinLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: ?Sized> DerefMut for SpinLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
