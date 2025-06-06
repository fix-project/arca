pub mod initcell;
pub mod rwlock;
pub mod spinlock;

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
