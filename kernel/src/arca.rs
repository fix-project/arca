use alloc::vec::Vec;

use crate::{
    cpu::{Cpu, ExitStatus, RegisterFile},
    paging::{
        PageTable, PageTable1GB, PageTable256TB, PageTable256TBEntry, PageTable2MB, PageTable512GB,
        PageTableEntry, Permissions, UnmappedPage,
    },
    refcnt::{SharedPage, SharedPage4KB},
};

#[derive(Clone)]
pub struct Blob {
    pub pages: Vec<SharedPage4KB>,
}

pub struct Arca {
    page_table: SharedPage<PageTable256TB>,
    register_file: RegisterFile,
}

impl Arca {
    pub fn new() -> Arca {
        let mut page_table = PageTable256TB::new();
        let mut pdpt = crate::rsstart::KERNEL_PAGES.lock().clone();
        pdpt.make_mut()[0].protect(Permissions::All);
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
        &mut self.page_table.make_mut()[0]
    }

    pub fn map(&mut self, blob: &Blob, addr: usize) {
        assert!(addr % 4096 == 0);
        for i in 0..blob.pages.len() {
            let current = addr + i * 4096;
            let mut pdpt = match self.page_table.make_mut()[(current >> 39) & 255].unmap() {
                UnmappedPage::Table(pdpt) => pdpt,
                _ => PageTable512GB::new().into(),
            };
            let mut pd = match pdpt.make_mut()[(current >> 30) & 511].unmap() {
                UnmappedPage::Table(pt) => pt,
                _ => PageTable1GB::new().into(),
            };
            let mut pt = match pd.make_mut()[(current >> 21) & 511].unmap() {
                UnmappedPage::Table(pt) => pt,
                _ => PageTable2MB::new().into(),
            };
            let page = blob.pages[i].clone();
            pt.make_mut()[(current >> 12) & 511].map_shared(page);
            pd.make_mut()[(current >> 21) & 511].chain(pt);
            pdpt.make_mut()[(current >> 30) & 511].chain(pd);
            self.page_table.make_mut()[(current >> 39) & 255].chain(pdpt);
        }
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
                .activate_page_table(crate::rsstart::PAGE_MAP.lock().clone())
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

    pub fn map(&mut self, blob: &Blob, addr: usize) {
        assert!(addr % 4096 == 0);
        unsafe {
            self.cpu.modify_page_table(|pml4| {
                for i in 0..blob.pages.len() {
                    let current = addr + i * 4096;
                    let mut pdpt = match pml4.make_mut()[(current >> 39) & 255].unmap() {
                        UnmappedPage::Table(pdpt) => pdpt,
                        _ => PageTable512GB::new().into(),
                    };
                    let mut pd = match pdpt.make_mut()[(current >> 30) & 511].unmap() {
                        UnmappedPage::Table(pt) => pt,
                        _ => PageTable1GB::new().into(),
                    };
                    let mut pt = match pd.make_mut()[(current >> 21) & 511].unmap() {
                        UnmappedPage::Table(pt) => pt,
                        _ => PageTable2MB::new().into(),
                    };
                    let page = blob.pages[i].clone();
                    pt.make_mut()[(current >> 12) & 511].map_shared(page);
                    pd.make_mut()[(current >> 21) & 511].chain(pt);
                    pdpt.make_mut()[(current >> 30) & 511].chain(pd);
                    pml4.make_mut()[(current >> 39) & 255].chain(pdpt);
                }
            });
        }
    }
}
