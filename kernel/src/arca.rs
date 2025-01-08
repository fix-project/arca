use alloc::{boxed::Box, vec::Vec};
use common::BuddyAllocator;

use crate::{
    cpu::{Cpu, ExitStatus, RegisterFile},
    page::{Page4KB, SharedPage},
    paging::{
        PageTable, PageTable1GB, PageTable256TB, PageTable256TBEntry, PageTable2MB, PageTable512GB,
        PageTableEntry, UnmappedPage,
    },
};

#[derive(Clone)]
pub struct Blob {
    pub pages: Vec<SharedPage<Page4KB>>,
}

impl Blob {
    pub fn len(&self) -> usize {
        self.pages.len() * 4096
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

pub struct MapConfig {
    pub offset: usize,
    pub addr: usize,
    pub len: usize,
    pub unique: bool,
}

pub trait AddressSpace {
    fn with_page_table_mut(&mut self, f: impl FnOnce(&mut SharedPage<PageTable256TB>));

    fn map_blob(&mut self, blob: &Blob, config: MapConfig) {
        let MapConfig {
            offset,
            addr,
            len,
            unique,
        } = config;
        assert!(addr % 4096 == 0);
        assert!(offset % 4096 == 0);
        assert!(len % 4096 == 0);
        self.with_page_table_mut(|pml4| {
            let mut pml4i = (addr >> 39) & 255;
            let mut pdpti = (addr >> 30) & 511;
            let mut pdi = (addr >> 21) & 511;
            let mut pti = (addr >> 12) & 511;

            let mut pdpt = match SharedPage::make_mut(pml4)[pml4i].unmap() {
                UnmappedPage::Table(pdpt) => pdpt,
                _ => PageTable512GB::new().into(),
            };
            let mut pd = match SharedPage::make_mut(&mut pdpt)[pdpti].unmap() {
                UnmappedPage::Table(pt) => pt,
                _ => PageTable1GB::new().into(),
            };
            let mut pt = match SharedPage::make_mut(&mut pd)[pdi].unmap() {
                UnmappedPage::Table(pt) => pt,
                _ => PageTable2MB::new().into(),
            };

            for page in &blob.pages[offset / 4096..(offset + len) / 4096] {
                let pte = &mut SharedPage::make_mut(&mut pt)[pti];
                if unique {
                    let allocator = crate::allocator::PHYSICAL_ALLOCATOR.lock();
                    let allocator: &BuddyAllocator<'static> =
                        unsafe { core::mem::transmute(allocator.get().unwrap()) };
                    let clone = Box::new_in(**page, allocator);
                    pte.map_unique(clone);
                } else {
                    pte.map_shared(page.clone());
                }

                pti += 1;
                if pti >= 512 {
                    pti = 0;
                    pdi += 1;
                    SharedPage::make_mut(&mut pd)[pdi].chain(pt);
                    pt = match SharedPage::make_mut(&mut pd)[pdi].unmap() {
                        UnmappedPage::Table(pt) => pt,
                        _ => PageTable2MB::new().into(),
                    };
                }
                if pdi >= 512 {
                    pdi = 0;
                    pdpti += 1;
                    SharedPage::make_mut(&mut pdpt)[pdpti].chain(pd);
                    pd = match SharedPage::make_mut(&mut pdpt)[pdpti].unmap() {
                        UnmappedPage::Table(pt) => pt,
                        _ => PageTable1GB::new().into(),
                    };
                }
                if pdpti >= 512 {
                    pdpti = 0;
                    pml4i += 1;
                    SharedPage::make_mut(pml4)[pml4i].chain(pdpt);
                    pdpt = match SharedPage::make_mut(pml4)[pml4i].unmap() {
                        UnmappedPage::Table(pdpt) => pdpt,
                        _ => PageTable512GB::new().into(),
                    };
                }
                if pml4i >= 256 {
                    panic!("attempted to map into kernel space");
                }
            }

            SharedPage::make_mut(&mut pd)[pdi].chain(pt);
            SharedPage::make_mut(&mut pdpt)[pdpti].chain(pd);
            SharedPage::make_mut(pml4)[pml4i].chain(pdpt);
        });
    }

    fn unmap(&mut self, addr: usize, len: usize) {
        assert!(addr % 4096 == 0);
        assert!(len % 4096 == 0);
        self.with_page_table_mut(|pml4| {
            let mut pml4i = (addr >> 39) & 255;
            let mut pdpti = (addr >> 30) & 511;
            let mut pdi = (addr >> 21) & 511;
            let mut pti = (addr >> 12) & 511;

            let mut pdpt = match SharedPage::make_mut(pml4)[pml4i].unmap() {
                UnmappedPage::Table(pdpt) => pdpt,
                _ => PageTable512GB::new().into(),
            };
            let mut pd = match SharedPage::make_mut(&mut pdpt)[pdpti].unmap() {
                UnmappedPage::Table(pt) => pt,
                _ => PageTable1GB::new().into(),
            };
            let mut pt = match SharedPage::make_mut(&mut pd)[pdi].unmap() {
                UnmappedPage::Table(pt) => pt,
                _ => PageTable2MB::new().into(),
            };

            for _ in 0..len / 4096 {
                let pte = &mut SharedPage::make_mut(&mut pt)[pti];
                pte.unmap();

                pti += 1;
                if pti >= 512 {
                    pti = 0;
                    pdi += 1;
                    SharedPage::make_mut(&mut pd)[pdi].chain(pt);
                    pt = match SharedPage::make_mut(&mut pd)[pdi].unmap() {
                        UnmappedPage::Table(pt) => pt,
                        _ => PageTable2MB::new().into(),
                    };
                }
                if pdi >= 512 {
                    pdi = 0;
                    pdpti += 1;
                    SharedPage::make_mut(&mut pdpt)[pdpti].chain(pd);
                    pd = match SharedPage::make_mut(&mut pdpt)[pdpti].unmap() {
                        UnmappedPage::Table(pt) => pt,
                        _ => PageTable1GB::new().into(),
                    };
                }
                if pdpti >= 512 {
                    pdpti = 0;
                    pml4i += 1;
                    SharedPage::make_mut(pml4)[pml4i].chain(pdpt);
                    pdpt = match SharedPage::make_mut(pml4)[pml4i].unmap() {
                        UnmappedPage::Table(pdpt) => pdpt,
                        _ => PageTable512GB::new().into(),
                    };
                }
                if pml4i >= 256 {
                    panic!("attempted to map into kernel space");
                }
            }

            SharedPage::make_mut(&mut pd)[pdi].chain(pt);
            SharedPage::make_mut(&mut pdpt)[pdpti].chain(pd);
            SharedPage::make_mut(pml4)[pml4i].chain(pdpt);
        });
    }
}

#[derive(Clone)]
pub struct Arca {
    page_table: SharedPage<PageTable256TB>,
    register_file: RegisterFile,
}

impl Arca {
    pub fn new() -> Arca {
        let mut page_table = PageTable256TB::new();
        let pdpt = crate::rsstart::KERNEL_MAPPINGS.lock().clone();
        page_table[256].chain(pdpt);

        let register_file = RegisterFile::new();
        Arca {
            page_table: page_table.into(),
            register_file,
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        unsafe { cpu.activate_page_table(self.page_table) };
        LoadedArca {
            register_file: self.register_file,
            cpu,
        }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn mappings(&self) -> &PageTable256TBEntry {
        &self.page_table[0]
    }

    pub fn mappings_mut(&mut self) -> &mut PageTable256TBEntry {
        &mut SharedPage::make_mut(&mut self.page_table)[0]
    }
}

impl AddressSpace for Arca {
    fn with_page_table_mut(&mut self, f: impl FnOnce(&mut SharedPage<PageTable256TB>)) {
        f(&mut self.page_table);
    }
}

impl Default for Arca {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LoadedArca<'a> {
    register_file: RegisterFile,
    cpu: &'a mut Cpu,
}

impl LoadedArca<'_> {
    pub fn run(&mut self) -> ExitStatus {
        unsafe { self.cpu.run(&mut self.register_file) }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn unload(self) -> Arca {
        let page_table = unsafe {
            self.cpu
                .activate_page_table(crate::rsstart::KERNEL_PAGE_MAP.lock().clone())
                .unwrap()
        };

        Arca {
            register_file: self.register_file,
            page_table,
        }
    }

    pub fn swap(&mut self, other: &mut Arca) {
        other.page_table = unsafe {
            self.cpu
                .activate_page_table(other.page_table.clone())
                .unwrap()
        };
        core::mem::swap(&mut self.register_file, &mut other.register_file);
    }
}

impl<'a> AddressSpace for LoadedArca<'a> {
    fn with_page_table_mut(&mut self, f: impl FnOnce(&mut SharedPage<PageTable256TB>)) {
        unsafe {
            self.cpu.modify_page_table(f);
        }
    }
}
