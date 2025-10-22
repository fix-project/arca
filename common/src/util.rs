// pub mod channel;
pub mod descriptors;
pub mod initcell;
// pub mod mutex;
pub mod concurrent_trie;
pub mod oneshot;
pub mod router;
pub mod rwlock;
pub mod semaphore;
pub mod sorter;
pub mod spinlock;

pub mod mutex {
    pub use async_lock::*;
}

pub mod channel {
    pub use async_channel::*;
}

pub fn replace_with<T>(x: &mut T, f: impl FnOnce(T) -> T) {
    unsafe {
        let old = core::ptr::read(x);
        let new = f(old);
        core::ptr::write(x, new);
    }
}

pub fn try_replace_with<T, E>(x: &mut T, f: impl FnOnce(T) -> Result<T, E>) -> Result<(), E> {
    unsafe {
        let old = core::ptr::read(x);
        let new = f(old)?;
        core::ptr::write(x, new);
        Ok(())
    }
}
