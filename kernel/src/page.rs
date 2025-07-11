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
pub type SharedPage<T> = RefCnt<T>;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum CowPage<T> {
    Unique(UniquePage<T>),
    Shared(SharedPage<T>),
}

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
            CowPage::Shared(page) => (false, RefCnt::into_raw(page)),
        }
    }

    /// # Safety
    ///
    /// The arguments to this function must have come from [into_raw].
    pub unsafe fn from_raw(unique: bool, ptr: *mut T) -> Self {
        if unique {
            CowPage::Unique(Box::from_raw_in(ptr, BuddyAllocator))
        } else {
            CowPage::Shared(RefCnt::from_raw(ptr))
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
