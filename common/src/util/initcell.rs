use core::{
    cell::UnsafeCell,
    mem::{ManuallyDrop, MaybeUninit},
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

union Lazy<T, F = fn() -> T> {
    thunk: ManuallyDrop<F>,
    value: ManuallyDrop<T>,
}

pub struct LazyLock<T, F = fn() -> T> {
    stat: AtomicUsize,
    data: UnsafeCell<Lazy<T, F>>,
}

unsafe impl<T: Send + Sync, F: Send> Sync for LazyLock<T, F> {}

impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    pub const fn new(f: F) -> LazyLock<T, F> {
        LazyLock {
            stat: AtomicUsize::new(0),
            data: UnsafeCell::new(Lazy {
                thunk: ManuallyDrop::new(f),
            }),
        }
    }

    pub fn force(this: &Self) -> &T {
        if this
            .stat
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // this thread should initialize
            let thunk = unsafe { ManuallyDrop::take(&mut (*this.data.get()).thunk) };
            let value = thunk();
            unsafe {
                (*this.data.get()).value = ManuallyDrop::new(value);
            }
            this.stat.store(2, Ordering::Release);
            unsafe { &(*this.data.get()).value }
        } else {
            loop {
                if let Some(x) = LazyLock::get(this) {
                    break x;
                }
                core::hint::spin_loop();
            }
        }
    }

    pub fn get(this: &Self) -> Option<&T> {
        if this.stat.load(Ordering::Acquire) == 2 {
            Some(unsafe { &(*this.data.get()).value })
        } else {
            None
        }
    }
}

impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        LazyLock::<T, F>::force(self)
    }
}

impl<T: Default> Default for LazyLock<T, fn() -> T> {
    fn default() -> Self {
        Self::new(Default::default)
    }
}

impl<T, F> Drop for LazyLock<T, F> {
    fn drop(&mut self) {
        todo!("implement Drop for OnceLock");
    }
}

pub struct OnceLock<T> {
    stat: AtomicUsize,
    data: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for OnceLock<T> {}
unsafe impl<T: Send + Sync> Sync for OnceLock<T> {}

impl<T> OnceLock<T> {
    pub const fn new() -> OnceLock<T> {
        OnceLock {
            stat: AtomicUsize::new(0),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub fn set(&self, value: T) -> Result<(), T> {
        if self
            .stat
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // this thread should initialize
            unsafe {
                (*self.data.get()).write(value);
            }
            self.stat.store(2, Ordering::Release);
            Ok(())
        } else {
            Err(value)
        }
    }

    pub fn get(&self) -> Option<&T> {
        if self.stat.load(Ordering::Acquire) == 2 {
            unsafe { Some((*self.data.get()).assume_init_ref()) }
        } else {
            None
        }
    }

    pub fn get_or_init(&self, f: impl FnOnce() -> T) -> &T {
        if self
            .stat
            .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            // this thread should initialize
            unsafe {
                (*self.data.get()).write(f());
            }
            self.stat.store(2, Ordering::Release);
        }
        self.wait()
    }

    pub fn wait(&self) -> &T {
        loop {
            if let Some(x) = self.get() {
                return x;
            }
            core::hint::spin_loop();
        }
    }
}

impl<T> Deref for OnceLock<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
            .expect("attempted to dereference OnceLock before initialization")
    }
}

impl<T: Default> Default for OnceLock<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for OnceLock<T> {
    fn drop(&mut self) {
        todo!("implement Drop for OnceLock");
    }
}
