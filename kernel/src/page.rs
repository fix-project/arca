use core::{
    cell::RefCell,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use crate::{
    arrayvec::ArrayVec,
    buddy::{allocate, liberate, BuddyAllocator},
};

#[repr(transparent)]
#[derive(Debug)]
pub struct UniquePage<T>(*mut T);

#[core_local]
static PAGE_CACHE: RefCell<ArrayVec<NonNull<MaybeUninit<[u8; 1 << 12]>>, 16>> =
    RefCell::new(ArrayVec::new());

impl<T> UniquePage<T> {
    const CACHEABLE: bool = BuddyAllocator::allocation_size::<T>()
        == BuddyAllocator::allocation_size::<[u8; 1 << 12]>()
        && BuddyAllocator::allocation_align::<T>()
            == BuddyAllocator::allocation_align::<[u8; 1 << 12]>();

    fn allocate() -> *mut MaybeUninit<T> {
        let allocation = if Self::CACHEABLE {
            let mut cache = PAGE_CACHE.borrow_mut();
            if cache.is_empty() {
                while cache.len() < cache.capacity() / 2 + 1 {
                    if let Some(allocation) = allocate::<[u8; 1 << 12]>() {
                        cache.push(allocation).unwrap();
                    } else {
                        break;
                    }
                }
            }
            let result: Option<NonNull<MaybeUninit<T>>> =
                unsafe { core::mem::transmute(cache.pop()) };
            result
        } else {
            allocate::<T>()
        };
        allocation
            .expect("could not allocate: physical memory exhausted")
            .as_ptr()
    }

    pub fn uninit() -> UniquePage<MaybeUninit<T>> {
        UniquePage(Self::allocate())
    }

    pub fn zeroed() -> UniquePage<MaybeUninit<T>> {
        let page = Self::allocate();
        unsafe { (*page).write(core::mem::zeroed()) };
        UniquePage(page)
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

impl<T> From<T> for UniquePage<T> {
    fn from(value: T) -> Self {
        let page = Self::allocate();
        let page: *mut T = unsafe {
            (*page).write(value);
            (*page).assume_init_mut()
        };
        UniquePage(page)
    }
}

impl<T: Default> Default for UniquePage<T> {
    fn default() -> Self {
        let default: T = Default::default();
        Self::from(default)
    }
}

impl<T: Copy, const N: usize> UniquePage<[T; N]> {
    pub fn new_cloned(value: T) -> Self {
        let page = Self::allocate();
        let page: *mut [T; N] = unsafe {
            let page: *mut [MaybeUninit<T>; N] = core::mem::transmute(page);
            (*page).fill(MaybeUninit::new(value));
            core::mem::transmute(page)
        };
        UniquePage(page)
    }
}

impl<const N: usize> UniquePage<[u8; N]> {
    pub fn new() -> Self {
        unsafe { Self::uninit().assume_init() }
    }
}

impl<T> UniquePage<MaybeUninit<T>> {
    /// # Safety
    /// This page's memory must be initialized to a valid bitpattern for T.
    pub unsafe fn assume_init(self) -> UniquePage<T> {
        let ptr = self.0;
        core::mem::forget(self);
        UniquePage(core::mem::transmute::<*mut MaybeUninit<T>, *mut T>(ptr))
    }

    pub fn write(self, value: T) -> UniquePage<T> {
        unsafe {
            (*self.0).write(value);
            self.assume_init()
        }
    }
}

impl<T> Deref for UniquePage<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<T> DerefMut for UniquePage<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<T> Drop for UniquePage<T> {
    fn drop(&mut self) {
        unsafe {
            self.0.drop_in_place();
            if Self::CACHEABLE {
                let mut cache = PAGE_CACHE.borrow_mut();
                let untyped: *mut MaybeUninit<[u8; 1 << 12]> = core::mem::transmute(self.0);
                let ptr = NonNull::new_unchecked(untyped);
                cache.push(ptr).unwrap();
                if cache.is_full() {
                    while cache.len() >= cache.capacity() / 2 {
                        liberate(cache.pop().unwrap().as_ptr());
                    }
                }
            } else {
                liberate(self.0)
            }
        }
    }
}

impl<T: Clone> Clone for UniquePage<T> {
    fn clone(&self) -> Self {
        Self::uninit().write(unsafe { (*self.0).clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_uninit() {
        core::hint::black_box(UniquePage::<u8>::uninit());
    }

    #[test]
    pub fn test_new() {
        let p = UniquePage::<u8>::from(31);
        assert_eq!(*p, 31);
    }

    #[test]
    pub fn test_cloned() {
        let p = UniquePage::<[u8; 1024]>::new_cloned(31);
        assert_eq!(p[16], 31);
    }

    #[test]
    pub fn test_clone() {
        let p = UniquePage::<[u8; 1024]>::new_cloned(31);
        let p = p.clone();
        assert_eq!(p[16], 31);
    }

    #[test]
    pub fn test_alloc_many() {
        let mut v = ArrayVec::<_, 64>::new();
        (0..32).for_each(|_| {
            v.push(UniquePage::<u8>::from(31)).unwrap();
        });
    }

    #[bench]
    pub fn bench_alloc_cached(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            core::hint::black_box(UniquePage::<u8>::from(31));
        })
    }

    #[repr(C, align(8192))]
    struct Weird(u8);

    #[bench]
    pub fn bench_alloc_uncached(bench: impl FnOnce(&dyn Fn())) {
        bench(&|| {
            core::hint::black_box(UniquePage::<Weird>::uninit());
        })
    }
}
