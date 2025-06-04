use common::message::Handle;

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
}

impl arca::RuntimeType for Page {
    type Runtime = Runtime;
}

impl arca::ValueType for Page {
    const DATATYPE: DataType = DataType::Page;
}

impl arca::Page for Page {
    fn read(&self, offset: usize, buffer: &mut [u8]) {
        let len = core::cmp::min(buffer.len(), self.size() - offset);
        let src = match self {
            Page::Page4KB(page) => &page[..],
            Page::Page2MB(page) => &page[..],
            Page::Page1GB(page) => &page[..],
        };
        buffer[..len].copy_from_slice(&src[offset..offset + len]);
    }

    fn write(&mut self, offset: usize, buffer: &[u8]) {
        let len = core::cmp::min(buffer.len(), self.size() - offset);
        let dst = match self {
            Page::Page4KB(page) => &mut page[..],
            Page::Page2MB(page) => &mut page[..],
            Page::Page1GB(page) => &mut page[..],
        };
        dst[offset..offset + len].copy_from_slice(buffer);
    }

    fn size(&self) -> usize {
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

impl TryFrom<Handle> for Page {
    type Error = Handle;

    fn try_from(value: Handle) -> Result<Self, Self::Error> {
        if value.datatype() == <Self as arca::ValueType>::DATATYPE {
            let (ptr, size) = value.read();
            let unique = size & 1 == 1;
            let size = size & !1;
            unsafe {
                Ok(match size {
                    val if val == 1 << 12 => {
                        Page::Page4KB(CowPage::from_raw(unique, ptr as *mut _))
                    }
                    val if val == 1 << 21 => {
                        Page::Page2MB(CowPage::from_raw(unique, ptr as *mut _))
                    }
                    val if val == 1 << 30 => {
                        Page::Page1GB(CowPage::from_raw(unique, ptr as *mut _))
                    }
                    _ => unreachable!(),
                })
            }
        } else {
            Err(value)
        }
    }
}

impl From<Page> for Handle {
    fn from(value: Page) -> Self {
        let mut size = value.size();
        let (unique, ptr) = match value {
            Page::Page4KB(page) => {
                let (unique, ptr) = CowPage::into_raw(page);
                (unique, ptr as usize)
            }
            Page::Page2MB(page) => {
                let (unique, ptr) = CowPage::into_raw(page);
                (unique, ptr as usize)
            }
            Page::Page1GB(page) => {
                let (unique, ptr) = CowPage::into_raw(page);
                (unique, ptr as usize)
            }
        };
        if unique {
            size |= 1;
        }
        Handle::new(DataType::Page, (ptr, size))
    }
}
