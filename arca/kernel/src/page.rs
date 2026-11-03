use core::ops::{Deref, DerefMut};

use crate::prelude::*;
use common::BuddyAllocator;

pub trait HardwarePage: Sized + Clone {}
impl HardwarePage for Page4KB {}
impl HardwarePage for Page2MB {}
impl HardwarePage for Page1GB {}
impl HardwarePage for ! {}

pub type Page4KB = [u8; 1 << 12];
pub type Page2MB = [u8; 1 << 21];
pub type Page1GB = [u8; 1 << 30];

pub type UniquePage<T> = Box<T, BuddyAllocator>;
#[repr(align(4096))]
struct ZeroPage(Page4KB);

static ZERO_PAGE: ZeroPage = ZeroPage([0; 4096]);

pub struct SharedPage<T> {
    pointer: *mut T,
}

unsafe impl<T: Send + Sync> Send for SharedPage<T> {}
unsafe impl<T: Send + Sync> Sync for SharedPage<T> {}

impl SharedPage<Page4KB> {
    pub fn zero() -> Self {
        Self {
            pointer: (&raw const ZERO_PAGE.0).cast_mut(),
        }
    }
}

impl<T> SharedPage<T> {
    fn is_zero(&self) -> bool {
        self.pointer.cast::<u8>() == (&raw const ZERO_PAGE.0).cast::<u8>().cast_mut()
    }

    pub fn into_raw(this: Self) -> *mut T {
        let pointer = this.pointer;
        core::mem::forget(this);
        pointer
    }

    /// # Safety
    /// `pointer` must own a reference returned by `into_raw`.
    pub unsafe fn from_raw(pointer: *mut T) -> Self {
        Self { pointer }
    }

    /// # Safety
    /// `pointer` must own a reference returned by `into_raw`.
    pub unsafe fn from_raw_in(pointer: *mut T, _: BuddyAllocator) -> Self {
        unsafe { Self::from_raw(pointer) }
    }
}

impl<T: Clone> SharedPage<T> {
    pub fn into_unique_retained(this: Self) -> (UniquePage<T>, Option<Self>) {
        if this.is_zero() {
            let unique = unsafe { UniquePage::<T>::new_zeroed_in(BuddyAllocator).assume_init() };
            (unique, None)
        } else {
            let (unique, retained) =
                unsafe { RefCnt::into_unique_retained(RefCnt::from_raw(Self::into_raw(this))) };
            let retained = retained.map(|page| unsafe { Self::from_raw(RefCnt::into_raw(page)) });
            (unique, retained)
        }
    }

    pub fn into_unique(this: Self) -> UniquePage<T> {
        if this.is_zero() {
            // Only the immutable, all-zero page can bypass reference counting.
            unsafe { UniquePage::<T>::new_zeroed_in(BuddyAllocator).assume_init() }
        } else {
            unsafe { RefCnt::into_unique(RefCnt::from_raw(Self::into_raw(this))) }
        }
    }
}

impl<T> Clone for SharedPage<T> {
    fn clone(&self) -> Self {
        if !self.is_zero() {
            let original = core::mem::ManuallyDrop::new(unsafe { RefCnt::from_raw(self.pointer) });
            let _ = RefCnt::into_raw(RefCnt::clone(&original));
        }
        Self {
            pointer: self.pointer,
        }
    }
}

impl<T> Drop for SharedPage<T> {
    fn drop(&mut self) {
        if !self.is_zero() {
            drop(unsafe { RefCnt::from_raw(self.pointer) });
        }
    }
}

impl<T> Deref for SharedPage<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.pointer }
    }
}

impl<T> From<UniquePage<T>> for SharedPage<T> {
    fn from(page: UniquePage<T>) -> Self {
        Self {
            pointer: RefCnt::into_raw(page.into()),
        }
    }
}

impl<T: core::fmt::Debug> core::fmt::Debug for SharedPage<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(&**self, f)
    }
}

impl<T: PartialEq> PartialEq for SharedPage<T> {
    fn eq(&self, other: &Self) -> bool {
        self.pointer == other.pointer || **self == **other
    }
}
impl<T: Eq> Eq for SharedPage<T> {}

#[derive(Debug, Clone)]
pub enum CowPage<T> {
    Unique(UniquePage<T>),
    Shared(SharedPage<T>),
}

impl<T: PartialEq> PartialEq for CowPage<T> {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(&**self, &**other) || **self == **other
    }
}
impl<T: Eq> Eq for CowPage<T> {}

impl<T> CowPage<T> {
    pub fn new() -> Self {
        unsafe { CowPage::Unique(UniquePage::new_zeroed_in(BuddyAllocator).assume_init()) }
    }

    pub fn unique(self) -> UniquePage<T>
    where
        T: Clone,
    {
        match self {
            CowPage::Unique(page) => page,
            CowPage::Shared(page) => SharedPage::into_unique(page),
        }
    }

    pub fn shared(self) -> SharedPage<T> {
        match self {
            CowPage::Unique(page) => page.into(),
            CowPage::Shared(page) => page,
        }
    }

    pub fn make_unique(&mut self)
    where
        T: Clone,
    {
        common::util::replace_with(self, |mut this| {
            if let CowPage::Shared(page) = this {
                this = CowPage::Unique(SharedPage::into_unique(page))
            }
            this
        });
    }

    pub fn into_raw(this: Self) -> (bool, *mut T) {
        match this {
            CowPage::Unique(page) => (true, Box::into_raw_with_allocator(page).0),
            CowPage::Shared(page) => (false, SharedPage::into_raw(page)),
        }
    }

    /// # Safety
    ///
    /// The arguments to this function must have come from [into_raw].
    pub unsafe fn from_raw(unique: bool, ptr: *mut T) -> Self {
        unsafe {
            if unique {
                CowPage::Unique(Box::from_raw_in(ptr, BuddyAllocator))
            } else {
                CowPage::Shared(SharedPage::from_raw(ptr))
            }
        }
    }
}

impl<T> Default for CowPage<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Deref for CowPage<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            CowPage::Unique(page) => page,
            CowPage::Shared(page) => page,
        }
    }
}

impl<T: Clone> DerefMut for CowPage<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.make_unique();
        let CowPage::Unique(page) = self else {
            unreachable!();
        };
        &mut *page
    }
}

impl<T> From<UniquePage<T>> for CowPage<T> {
    fn from(value: UniquePage<T>) -> Self {
        CowPage::Unique(value)
    }
}

impl<T> From<SharedPage<T>> for CowPage<T> {
    fn from(value: SharedPage<T>) -> Self {
        CowPage::Shared(value)
    }
}
