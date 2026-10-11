use super::KThread;
use alloc::boxed::Box;
use common::util::spinlock::SpinLock;
use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Default)]
pub struct KMutex<T: ?Sized> {
    lock: AtomicBool,
    waiters: SpinLock<Option<Box<KThread>>>,
    data: UnsafeCell<T>,
}

#[derive(Debug)]
pub struct KMutexGuard<'a, T: ?Sized> {
    pub(super) lock: &'a KMutex<T>,
    data: &'a mut T,
}

impl<T: ?Sized> KMutex<T> {
    pub const fn new(data: T) -> KMutex<T>
    where
        T: Sized,
    {
        KMutex {
            lock: AtomicBool::new(false),
            waiters: SpinLock::new(None),
            data: UnsafeCell::new(data),
        }
    }

    pub fn try_lock(&self) -> Option<KMutexGuard<'_, T>> {
        if self
            .lock
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(KMutexGuard {
                lock: self,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }

    pub fn lock(&self) -> KMutexGuard<'_, T> {
        loop {
            if let Some(guard) = self.try_lock() {
                return guard;
            }
            super::wait_for_mutex(&self.lock, &self.waiters);
        }
    }

    pub fn get(&self) -> *mut T {
        self.data.get()
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.lock();
        f(&mut guard)
    }

    pub fn unlock(guard: KMutexGuard<'_, T>) {
        KMutexGuard::unlock(guard);
    }
}

impl<T: ?Sized> KMutexGuard<'_, T> {
    pub fn unlock(_: Self) {}
}

impl<T: ?Sized> Drop for KMutexGuard<'_, T> {
    fn drop(&mut self) {
        let mut waiters = self.lock.waiters.lock();
        self.lock.lock.store(false, Ordering::Release);
        if let Some(mut thread) = waiters.take() {
            *waiters = thread.next.take();
            drop(waiters);
            super::enqueue(thread);
        }
    }
}

unsafe impl<T: Send + ?Sized> Send for KMutex<T> {}
unsafe impl<T: Send + ?Sized> Sync for KMutex<T> {}

impl<T: ?Sized> !Send for KMutexGuard<'_, T> {}

impl<T: ?Sized> Deref for KMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: ?Sized> DerefMut for KMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
