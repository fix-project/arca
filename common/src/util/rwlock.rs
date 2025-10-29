use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Default)]
pub struct RwLock<T: ?Sized> {
    count: AtomicUsize,
    data: UnsafeCell<T>,
}

#[derive(Debug)]
pub struct ReadGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
    data: &'a T,
}

#[derive(Debug)]
pub struct WriteGuard<'a, T: ?Sized> {
    lock: &'a RwLock<T>,
    data: &'a mut T,
}

impl<T: ?Sized> RwLock<T> {
    pub const fn new(data: T) -> RwLock<T>
    where
        T: Sized,
    {
        RwLock {
            count: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    pub fn try_write(&self) -> Option<WriteGuard<'_, T>> {
        if self
            .count
            .compare_exchange(0, usize::MAX, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(WriteGuard {
                lock: self,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }

    pub fn try_lock(&self) -> Option<WriteGuard<'_, T>> {
        self.try_write()
    }

    pub fn write(&self) -> WriteGuard<'_, T> {
        loop {
            if let Some(guard) = self.try_write() {
                return guard;
            }
            core::hint::spin_loop();
        }
    }

    pub fn lock(&self) -> WriteGuard<'_, T> {
        self.write()
    }

    pub fn try_read(&self) -> Option<ReadGuard<'_, T>> {
        loop {
            let old = self.count.load(Ordering::SeqCst);
            if old == usize::MAX {
                return None;
            }
            if self
                .count
                .compare_exchange(old, old + 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Some(ReadGuard {
                    lock: self,
                    data: unsafe { &*self.data.get() },
                });
            }
            core::hint::spin_loop();
        }
    }

    pub fn read(&self) -> ReadGuard<'_, T> {
        loop {
            if let Some(guard) = self.try_read() {
                return guard;
            }
            core::hint::spin_loop();
        }
    }

    pub fn upgrade(guard: ReadGuard<'_, T>) -> WriteGuard<'_, T>
    where
        T: Sized,
    {
        ReadGuard::upgrade(guard)
    }

    pub fn try_upgrade(guard: ReadGuard<'_, T>) -> Option<WriteGuard<'_, T>>
    where
        T: Sized,
    {
        ReadGuard::try_upgrade(guard)
    }

    pub fn downgrade(guard: WriteGuard<'_, T>) -> ReadGuard<'_, T>
    where
        T: Sized,
    {
        WriteGuard::downgrade(guard)
    }

    pub fn unread(_: ReadGuard<'_, T>)
    where
        T: Sized,
    {
    }

    pub fn unwrite(_: WriteGuard<'_, T>)
    where
        T: Sized,
    {
    }

    pub fn unlock(_: WriteGuard<'_, T>)
    where
        T: Sized,
    {
    }

    pub fn get(&self) -> *mut T {
        self.data.get()
    }
}

unsafe impl<T: Send> Send for RwLock<T> {}
unsafe impl<T: Send> Sync for RwLock<T> {}

impl<'a, T: ?Sized> WriteGuard<'a, T> {
    pub fn unlock(_: Self) {}

    pub fn downgrade(this: Self) -> ReadGuard<'a, T> {
        this.lock.count.store(1, Ordering::SeqCst);
        let guard = ReadGuard {
            lock: this.lock,
            data: unsafe { &*this.lock.get() },
        };
        core::mem::forget(this);
        guard
    }
}

impl<T: ?Sized> Drop for WriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.count.store(0, Ordering::Release);
    }
}

impl<T: ?Sized> Deref for WriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<T: ?Sized> DerefMut for WriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

impl<'a, T> ReadGuard<'a, T> {
    pub fn unlock(_: Self) {}

    pub fn upgrade(this: Self) -> WriteGuard<'a, T> {
        let lock = this.lock;
        ReadGuard::unlock(this);
        lock.write()
    }

    pub fn try_upgrade(this: Self) -> Option<WriteGuard<'a, T>> {
        let lock = this.lock;
        ReadGuard::unlock(this);
        lock.try_write()
    }
}

impl<T: ?Sized> Drop for ReadGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.count.fetch_sub(1, Ordering::Release);
    }
}

impl<T: ?Sized> Deref for ReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_rwlock() {
        let lock = RwLock::new(0);
        let Some(write) = lock.try_write() else {
            panic!("unconstested write should have succeeded");
        };
        assert!(lock.try_write().is_none());
        assert!(lock.try_read().is_none());
        let read = WriteGuard::downgrade(write);
        assert!(lock.try_write().is_none());
        assert!(lock.try_read().is_some());
        let Some(read2) = lock.try_read() else {
            panic!("read should have succeeded");
        };
        ReadGuard::unlock(read);
        assert!(lock.try_write().is_none());
        assert!(lock.try_read().is_some());
        let write = ReadGuard::upgrade(read2);
        assert!(lock.try_write().is_none());
        assert!(lock.try_read().is_none());
        WriteGuard::unlock(write);
        assert!(lock.try_write().is_some());
        assert!(lock.try_read().is_some());
    }
}
