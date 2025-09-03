use super::semaphore::*;

use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
};

#[derive(Debug)]
pub struct Mutex<T: ?Sized> {
    sema: Semaphore,
    data: UnsafeCell<T>,
}

impl<T: Default> Default for Mutex<T> {
    fn default() -> Self {
        Self {
            sema: Semaphore::new(1),
            data: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct MutexGuard<'a, T: ?Sized> {
    lock: &'a Mutex<T>,
    data: &'a mut T,
}

impl<T: ?Sized> Mutex<T> {
    pub fn new(data: T) -> Mutex<T>
    where
        T: Sized,
    {
        Mutex {
            sema: Semaphore::new(1),
            data: UnsafeCell::new(data),
        }
    }

    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if self.sema.try_acquire(1) {
            Some(MutexGuard {
                lock: self,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }

    pub async fn lock(&self) -> MutexGuard<'_, T> {
        self.sema.acquire(1).await;
        MutexGuard {
            lock: self,
            data: unsafe { &mut *self.data.get() },
        }
    }

    pub fn spin_lock(&self) -> MutexGuard<'_, T> {
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

    pub async fn with<R, A: AsyncFnOnce(&mut T) -> R>(&self, f: A) -> R {
        let mut guard = self.lock().await;
        f(&mut guard).await
    }

    pub async fn spin_with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        let mut guard = self.spin_lock();
        f(&mut guard)
    }

    pub fn unlock(guard: MutexGuard<'_, T>) {
        MutexGuard::unlock(guard);
    }
}

impl<T: ?Sized> MutexGuard<'_, T> {
    pub fn unlock(_: Self) {}
}

impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.sema.release(1);
    }
}

unsafe impl<T: Send + ?Sized> Send for Mutex<T> {}
unsafe impl<T: Send + ?Sized> Sync for Mutex<T> {}

impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
