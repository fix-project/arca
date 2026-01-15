use core::ops::{Deref, DerefMut};

use crate::paging::Impossible;
use crate::prelude::*;

use crate::page::{CowPage, Page1GB, Page2MB, Page4KB};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Page {
    Page4KB(CowPage<Page4KB>),
    Page2MB(CowPage<Page2MB>),
    Page1GB(CowPage<Page1GB>),
}

impl Page {
    pub fn new(size: usize) -> Page {
        if size <= Page4KB::SIZE {
            Page::Page4KB(Default::default())
        } else if size <= Page2MB::SIZE {
            Page::Page2MB(Default::default())
        } else if size <= Page1GB::SIZE {
            Page::Page1GB(Default::default())
        } else {
            panic!();
        }
    }

    pub fn shared(self) -> Page {
        match self {
            Page::Page4KB(page) => Page::Page4KB(page.shared().into()),
            Page::Page2MB(page) => Page::Page2MB(page.shared().into()),
            Page::Page1GB(page) => Page::Page1GB(page.shared().into()),
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Page::Page4KB(_) => 1 << 12,
            Page::Page2MB(_) => 1 << 21,
            Page::Page1GB(_) => 1 << 30,
        }
    }
}

impl From<CowPage<Impossible>> for Page {
    fn from(_: CowPage<Impossible>) -> Self {
        unreachable!()
    }
}

impl From<CowPage<Page4KB>> for Page {
    fn from(value: CowPage<Page4KB>) -> Self {
        Page::Page4KB(value)
    }
}

impl From<CowPage<Page2MB>> for Page {
    fn from(value: CowPage<Page2MB>) -> Self {
        Page::Page2MB(value)
    }
}

impl From<CowPage<Page1GB>> for Page {
    fn from(value: CowPage<Page1GB>) -> Self {
        Page::Page1GB(value)
    }
}

impl TryFrom<Page> for CowPage<Page4KB> {
    type Error = Page;

    fn try_from(value: Page) -> Result<Self, Self::Error> {
        match value {
            Page::Page4KB(page) => Ok(page),
            _ => Err(value),
        }
    }
}

impl TryFrom<Page> for CowPage<Page2MB> {
    type Error = Page;

    fn try_from(value: Page) -> Result<Self, Self::Error> {
        match value {
            Page::Page2MB(page) => Ok(page),
            _ => Err(value),
        }
    }
}

impl TryFrom<Page> for CowPage<Page1GB> {
    type Error = Page;

    fn try_from(value: Page) -> Result<Self, Self::Error> {
        match value {
            Page::Page1GB(page) => Ok(page),
            _ => Err(value),
        }
    }
}

impl Deref for Page {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        match self {
            Page::Page4KB(page) => &page[..],
            Page::Page2MB(page) => &page[..],
            Page::Page1GB(page) => &page[..],
        }
    }
}

impl DerefMut for Page {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Page::Page4KB(page) => &mut page[..],
            Page::Page2MB(page) => &mut page[..],
            Page::Page1GB(page) => &mut page[..],
        }
    }
}
