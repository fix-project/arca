use alloc::{vec, vec::Vec};

use crate::{cpu::ExitStatus, prelude::*, rsstart::KERNEL_PAGE_MAP};

use super::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: SharedPage<PageTable256TB>,
    register_file: RegisterFile,
    descriptors: Vec<Value>,
}

impl Arca {
    pub fn new() -> Arca {
        let mut page_table = PageTable256TB::new();
        let pdpt = crate::rsstart::KERNEL_MAPPINGS.clone();
        page_table[256].chain(pdpt);

        let register_file = RegisterFile::new();
        Arca {
            page_table: page_table.into(),
            register_file,
            descriptors: vec![],
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        unsafe { cpu.activate_page_table(self.page_table) };
        LoadedArca {
            register_file: self.register_file,
            descriptors: self.descriptors,
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

    pub fn descriptors(&self) -> &Vec<Value> {
        &self.descriptors
    }

    pub fn descriptors_mut(&mut self) -> &mut Vec<Value> {
        &mut self.descriptors
    }
}

impl Default for Arca {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LoadedArca<'a> {
    register_file: RegisterFile,
    descriptors: Vec<Value>,
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

    pub fn descriptors(&self) -> &Vec<Value> {
        &self.descriptors
    }

    pub fn descriptors_mut(&mut self) -> &mut Vec<Value> {
        &mut self.descriptors
    }

    pub fn unload(self) -> Arca {
        let page_table = unsafe {
            self.cpu
                .activate_page_table(KERNEL_PAGE_MAP.clone())
                .unwrap()
        };

        Arca {
            register_file: self.register_file,
            descriptors: self.descriptors,
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
