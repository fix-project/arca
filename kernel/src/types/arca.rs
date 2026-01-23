use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use arcane::SyscallError;

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

use crate::types::internal;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: Table,
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    fsbase: u64,
    // rlimit: Resources,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resources {
    pub memory: usize,
}

impl Arca {
    pub fn new() -> Arca {
        let page_table = Table::from_inner(internal::Table::default());
        let register_file = RegisterFile::new().into();
        let descriptors = Descriptors::new();
        // let rlimit = Resources { memory: 1 << 21 };

        Arca {
            page_table,
            register_file,
            descriptors,
            fsbase: 0,
            // rlimit,
        }
    }

    pub fn new_with(
        register_file: impl Into<Box<RegisterFile>>,
        page_table: Table,
        descriptors: Tuple,
        _rlimit: Tuple,
    ) -> Arca {
        let descriptors = Vec::from(descriptors.into_inner().into_inner()).into();

        // let mem_limit = Word::try_from(rlimit.get(0).clone()).unwrap().read() as usize;
        // let rlimit = Resources { memory: mem_limit };

        Arca {
            page_table,
            register_file: register_file.into(),
            descriptors,
            fsbase: 0,
            // rlimit,
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        // let memory = ValueRef::Table(&self.page_table).byte_size()
        //     + self
        //         .descriptors
        //         .iter()
        //         .map(|x| x.byte_size())
        //         .reduce(|x, y| x + y)
        //         .unwrap();

        cpu.activate_address_space(self.page_table.into_inner());
        unsafe {
            core::arch::asm! {
                "wrfsbase {base}", base=in(reg) self.fsbase
            };
        }
        // let rusage = Resources { memory };
        // assert!(rusage.memory <= self.rlimit.memory);
        LoadedArca {
            register_file: self.register_file,
            descriptors: self.descriptors,
            cpu,
            // rlimit: self.rlimit,
            // rusage,
        }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn mappings(&self) -> &Table {
        &self.page_table
    }

    pub fn mappings_mut(&mut self) -> &mut Table {
        &mut self.page_table
    }

    pub fn descriptors(&self) -> &Descriptors {
        &self.descriptors
    }

    pub fn descriptors_mut(&mut self) -> &mut Descriptors {
        &mut self.descriptors
    }

    pub fn read(self) -> (RegisterFile, Table, Tuple) {
        (
            *self.register_file,
            self.page_table,
            Tuple::from_inner(internal::Tuple::new(Vec::from(self.descriptors))),
        )
    }

    // pub fn rlimit(&self) -> &Resources {
    //     &self.rlimit
    // }

    // pub fn rlimit_mut(&mut self) -> &mut Resources {
    //     &mut self.rlimit
    // }
}

impl Default for Arca {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct LoadedArca<'a> {
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    cpu: &'a mut Cpu,
    // rlimit: Resources,
    // rusage: Resources,
}

impl<'a> LoadedArca<'a> {
    pub fn run(&mut self) -> ExitReason {
        unsafe { self.cpu.run(&mut self.register_file) }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn descriptors(&self) -> &Descriptors {
        &self.descriptors
    }

    pub fn descriptors_mut(&'_ mut self) -> DescriptorsProxy<'_, 'a> {
        DescriptorsProxy { arca: self }
    }

    // pub fn rlimit(&self) -> &Resources {
    //     &self.rlimit
    // }

    // pub fn rlimit_mut(&mut self) -> &mut Resources {
    //     &mut self.rlimit
    // }

    // pub fn rusage(&self) -> &Resources {
    //     &self.rusage
    // }

    // pub fn rusage_mut(&mut self) -> &mut Resources {
    //     &mut self.rusage
    // }

    pub fn unload(self) -> Arca {
        self.unload_with_cpu().0
    }

    pub fn take(&mut self) -> Arca {
        let mut original = Default::default();
        self.swap(&mut original);
        original
    }

    pub fn unload_with_cpu(self) -> (Arca, &'a mut Cpu) {
        let page_table = Table::from_inner(self.cpu.deactivate_address_space());
        let mut fsbase: u64;
        unsafe {
            core::arch::asm! {
                "rdfsbase {base}", base=out(reg) fsbase
            };
        }

        (
            Arca {
                register_file: self.register_file,
                descriptors: self.descriptors,
                page_table,
                fsbase,
                // rlimit: self.rlimit,
            },
            self.cpu,
        )
    }

    pub fn swap(&mut self, other: &mut Arca) {
        core::mem::swap(&mut self.register_file, &mut other.register_file);
        core::mem::swap(&mut self.descriptors, &mut other.descriptors);
        // core::mem::swap(&mut self.rlimit, &mut other.rlimit);
        let mut fsbase: u64;
        unsafe {
            core::arch::asm!("rdfsbase {old}; wrfsbase {new}", old=out(reg) fsbase, new=in(reg) other.fsbase);
        }
        other.fsbase = fsbase;
        self.cpu.swap_address_space(other.page_table.inner_mut());
    }

    pub fn cpu(&'_ mut self) -> CpuProxy<'_, 'a> {
        CpuProxy { arca: self }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DescriptorError {
    AttemptToMutateNull,
    OutOfBounds,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptors {
    descriptors: Vec<Value>,
}

pub type Result<T> = core::result::Result<T, DescriptorError>;

impl Descriptors {
    pub fn new() -> Descriptors {
        Descriptors {
            descriptors: vec![Value::default()],
        }
    }

    pub fn get(&self, index: usize) -> Result<&Value> {
        self.descriptors
            .get(index)
            .ok_or(DescriptorError::OutOfBounds)
    }

    pub fn get_mut(&mut self, index: usize) -> Result<&mut Value> {
        if index == 0 {
            return Err(DescriptorError::AttemptToMutateNull);
        }
        self.descriptors
            .get_mut(index)
            .ok_or(DescriptorError::OutOfBounds)
    }

    pub fn take(&mut self, index: usize) -> Result<Value> {
        if index == 0 {
            return Ok(Value::default());
        }
        Ok(core::mem::take(
            self.descriptors
                .get_mut(index)
                .ok_or(DescriptorError::OutOfBounds)?,
        ))
    }

    pub fn insert(&mut self, value: Value) -> usize {
        if value.datatype() == DataType::Null {
            return 0;
        }
        for (i, x) in self.descriptors.iter_mut().enumerate().skip(1) {
            if x.datatype() == DataType::Null {
                *x = value;
                return i;
            }
        }
        let i = self.descriptors.len();
        self.descriptors.push(value);
        i
    }

    pub fn iter(&'_ self) -> Iter<'_> {
        Iter { i: 0, d: self }
    }
}

pub struct Iter<'a> {
    i: usize,
    d: &'a Descriptors,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Value;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.d.descriptors.len() {
            return None;
        }
        self.i += 1;
        Some(self.d.get(self.i - 1).unwrap())
    }
}

impl Default for Descriptors {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<Value>> for Descriptors {
    fn from(mut value: Vec<Value>) -> Self {
        if value.is_empty() {
            value.push(Default::default());
        } else {
            value[0] = Default::default();
        }
        Descriptors { descriptors: value }
    }
}

impl From<Descriptors> for Vec<Value> {
    fn from(value: Descriptors) -> Self {
        value.descriptors
    }
}

impl From<Arca> for super::Function {
    fn from(value: Arca) -> super::Function {
        super::Function::from_inner(super::internal::Function::arcane_with_args(
            value,
            VecDeque::default(),
        ))
    }
}

pub struct DescriptorsProxy<'a, 'cpu> {
    arca: &'a mut LoadedArca<'cpu>,
}

impl<'a> DescriptorsProxy<'a, '_> {
    pub fn insert(self, value: Value) -> core::result::Result<usize, SyscallError> {
        // let size = value.byte_size();
        // if self.arca.rusage().memory + size > self.arca.rlimit().memory {
        //     return Err(SyscallError::OutOfMemory);
        // }
        // self.arca.rusage_mut().memory += size;
        Ok(self.arca.descriptors.insert(value))
    }

    pub fn take(self, index: usize) -> core::result::Result<Value, SyscallError> {
        let value = self.arca.descriptors.take(index)?;
        // let size = value.byte_size();
        // self.arca.rusage_mut().memory -= size;
        Ok(value)
    }

    pub fn get(self, index: usize) -> core::result::Result<&'a Value, DescriptorError> {
        self.arca.descriptors.get(index)
    }

    pub fn get_mut(self, index: usize) -> core::result::Result<&'a mut Value, DescriptorError> {
        self.arca.descriptors.get_mut(index)
    }
}

pub struct CpuProxy<'a, 'cpu> {
    arca: &'a mut LoadedArca<'cpu>,
}

impl<'a> CpuProxy<'a, '_> {
    pub fn map(
        &mut self,
        address: usize,
        entry: Entry,
    ) -> core::result::Result<Entry, SyscallError> {
        // let limit = self.arca.rlimit().memory;
        // let new_size = entry.byte_size();
        // let old = self.arca.cpu.map(address, Entry::Null(new_size))?;
        // let old_size = old.byte_size();
        // let usage = &mut self.arca.rusage_mut().memory;
        // *usage -= old_size;
        // if *usage + new_size > limit {
        //     self.arca.cpu.map(address, old)?;
        //     self.arca.rusage_mut().memory += old_size;
        //     return Err(SyscallError::OutOfMemory);
        // }
        let old = self.arca.cpu.map(address, entry)?;
        // self.arca.rusage_mut().memory += new_size;
        Ok(old)
    }
}
