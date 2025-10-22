use alloc::{collections::vec_deque::VecDeque, vec::Vec};

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

use crate::types::internal::{Table, Tuple};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: Table,
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    fsbase: u64,
}

impl Arca {
    pub fn new() -> Arca {
        let page_table = Table::default();
        let register_file = RegisterFile::new().into();
        let descriptors = Descriptors::new();

        Arca {
            page_table,
            register_file,
            descriptors,
            fsbase: 0,
        }
    }

    pub fn new_with(
        register_file: impl Into<Box<RegisterFile>>,
        page_table: Table,
        descriptors: Tuple,
    ) -> Arca {
        let descriptors = Vec::from(descriptors.into_inner()).into();

        Arca {
            page_table,
            register_file: register_file.into(),
            descriptors,
            fsbase: 0,
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        cpu.activate_address_space(self.page_table);
        unsafe {
            core::arch::asm! {
                "wrfsbase {base}", base=in(reg) self.fsbase
            };
        }
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
            Tuple::new(Vec::from(self.descriptors)),
        )
    }
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

    pub fn descriptors_mut(&mut self) -> &mut Descriptors {
        &mut self.descriptors
    }

    pub fn unload(self) -> Arca {
        self.unload_with_cpu().0
    }

    pub fn take(&mut self) -> Arca {
        let mut original = Default::default();
        self.swap(&mut original);
        original
    }

    pub fn unload_with_cpu(self) -> (Arca, &'a mut Cpu) {
        let page_table = self.cpu.deactivate_address_space();
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
            },
            self.cpu,
        )
    }

    pub fn swap(&mut self, other: &mut Arca) {
        core::mem::swap(&mut self.register_file, &mut other.register_file);
        core::mem::swap(&mut self.descriptors, &mut other.descriptors);
        let mut fsbase: u64;
        unsafe {
            core::arch::asm!("rdfsbase {old}; wrfsbase {new}", old=out(reg) fsbase, new=in(reg) other.fsbase);
        }
        other.fsbase = fsbase;
        self.cpu.swap_address_space(&mut other.page_table);
    }

    pub fn cpu(&mut self) -> &'_ mut Cpu {
        self.cpu
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
