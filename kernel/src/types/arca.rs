use alloc::{vec, vec::Vec};

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: AddressSpace,
    register_file: RegisterFile,
    descriptors: Vec<Value>,
}

impl Arca {
    pub fn new() -> Arca {
        let page_table = AddressSpace::new();
        let register_file = RegisterFile::new();

        Arca {
            page_table,
            register_file,
            descriptors: vec![],
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        cpu.activate_address_space(self.page_table);
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

    // pub fn mappings(&self) -> &PageTableEntry<PageTable256TB> {
    //     &self.page_table[0]
    // }

    pub fn mappings_mut(&mut self) -> &mut AddressSpace {
        &mut self.page_table
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
    pub fn run(&mut self) -> ExitReason {
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
        let page_table = self.cpu.deactivate_address_space();

        Arca {
            register_file: self.register_file,
            descriptors: self.descriptors,
            page_table,
        }
    }
}
