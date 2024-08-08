use core::{
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
};

use crate::buddy::{allocate, liberate};

#[repr(transparent)]
#[derive(Debug)]
pub struct Page<T>(*mut T);

impl<T> Page<T> {
    fn allocate() -> *mut MaybeUninit<T> {
        allocate::<T>()
            .expect("could not allocate: physical memory exhausted")
            .as_ptr()
    }

    pub fn uninit() -> Page<MaybeUninit<T>> {
        Page(Self::allocate())
    }

    pub fn zeroed() -> Page<MaybeUninit<T>> {
        let page = Self::allocate();
        unsafe { (*page).write(core::mem::zeroed()) };
        Page(page)
    }

    pub fn as_ptr(&self) -> *mut T {
        self.0
    }

    pub fn into_raw(self) -> *mut T {
        let ptr = self.as_ptr();
        core::mem::forget(self);
        ptr
    }

    /// # Safety
    /// This pointer must have come from [into_raw] and must be unique.
    pub unsafe fn from_raw(ptr: *mut T) -> Self {
        Self(ptr)
    }
}

impl<T> From<T> for Page<T> {
    fn from(value: T) -> Self {
        let page = Self::allocate();
        let page: *mut T = unsafe {
            (*page).write(value);
            (*page).assume_init_mut()
        };
        Page(page)
    }
}

impl<T: Default> Default for Page<T> {
    fn default() -> Self {
        let default: T = Default::default();
        Self::from(default)
    }
}

impl<T: Copy, const N: usize> Page<[T; N]> {
    pub fn new_cloned(value: T) -> Self {
        let page = Self::allocate();
        let page: *mut [T; N] = unsafe {
            let page: *mut [MaybeUninit<T>; N] = core::mem::transmute(page);
            (*page).fill(MaybeUninit::new(value));
            core::mem::transmute(page)
        };
        Page(page)
    }
}

impl<const N: usize> Page<[u8; N]> {
    pub fn new() -> Self {
        unsafe { Self::uninit().assume_init() }
    }
}

impl<T> Page<MaybeUninit<T>> {
    /// # Safety
    /// This page's memory must be initialized to a valid bitpattern for T.
    pub unsafe fn assume_init(self) -> Page<T> {
        let ptr = self.0;
        core::mem::forget(self);
        Page(core::mem::transmute::<*mut MaybeUninit<T>, *mut T>(ptr))
    }

    pub fn write(self, value: T) -> Page<T> {
        unsafe {
            (*self.0).write(value);
            self.assume_init()
        }
    }
}

impl<T> Deref for Page<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T> DerefMut for Page<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<T> Drop for Page<T> {
    fn drop(&mut self) {
        unsafe {
            self.0.drop_in_place();
            liberate(self.0)
        }
    }
}

impl<T: Clone> Clone for Page<T> {
    fn clone(&self) -> Self {
        Self::uninit().write(unsafe { (*self.0).clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_uninit() {
        let _ = Page::<u8>::uninit();
    }

    #[test]
    pub fn test_new() {
        let p = Page::<u8>::from(31);
        assert_eq!(*p, 31);
    }

    #[test]
    pub fn test_cloned() {
        let p = Page::<[u8; 1024]>::new_cloned(31);
        assert_eq!(p[16], 31);
    }

    #[test]
    pub fn test_clone() {
        let p = Page::<[u8; 1024]>::new_cloned(31);
        let p = p.clone();
        assert_eq!(p[16], 31);
    }
}
