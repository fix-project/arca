use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    ops::Deref,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct InitCell<T, F = fn() -> T> {
    stat: AtomicUsize,
    init: UnsafeCell<Option<F>>,
    data: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send, F: FnOnce() -> T> Send for InitCell<T, F> {}
unsafe impl<T: Send + Sync, F: FnOnce() -> T> Sync for InitCell<T, F> {}

impl<T, F: FnOnce() -> T> InitCell<T, F> {
    pub const fn new(f: F) -> InitCell<T, F> {
        InitCell {
            stat: AtomicUsize::new(0),
            init: UnsafeCell::new(Some(f)),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub const fn empty() -> InitCell<T, F> {
        InitCell {
            stat: AtomicUsize::new(0),
            init: UnsafeCell::new(None),
            data: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    pub fn initialize<G: FnOnce() -> T>(this: &Self, f: G) -> &T {
        loop {
            match this
                .stat
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(0) => break,
                Err(1) => core::hint::spin_loop(),
                Err(2) => return unsafe { (*this.data.get()).assume_init_ref() },
                Ok(x) => unreachable!("invalid value for InitCell.stat: {x}"),
                Err(x) => unreachable!("invalid value for InitCell.stat: {x}"),
            }
        }
        let data = unsafe { &mut *this.data.get() };
        let init = unsafe { &mut *this.init.get() };
        init.take();
        let data = data.write(f());
        this.stat.store(2, Ordering::SeqCst);
        data
    }

    pub fn force(this: &Self) -> Option<&T> {
        loop {
            match this
                .stat
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(0) => break,
                Err(1) => core::hint::spin_loop(),
                Err(2) => return Some(unsafe { (*this.data.get()).assume_init_ref() }),
                Ok(x) => unreachable!("invalid value for InitCell.stat: {x}"),
                Err(x) => unreachable!("invalid value for InitCell.stat: {x}"),
            }
        }
        let data = unsafe { &mut *this.data.get() };
        let init = unsafe { &mut *this.init.get() };
        let init = init.take()?;
        let data = data.write(init());
        this.stat.store(2, Ordering::SeqCst);
        Some(data)
    }
}

impl<T, F: FnOnce() -> T> Deref for InitCell<T, F> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        InitCell::force(self)
            .expect("attempted to dereference uninitialized InitCell with no initializer")
    }
}

impl<T, F: FnOnce() -> T> Default for InitCell<T, F> {
    fn default() -> Self {
        Self::empty()
    }
}
