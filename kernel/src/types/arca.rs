use alloc::vec::Vec;

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

use crate::types::internal::Table;
use crate::types::internal::Tuple;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: Table,
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    error_buffer: Option<String>,
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
            error_buffer: None,
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
            error_buffer: None,
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        cpu.activate_address_space(self.page_table);
        LoadedArca {
            register_file: self.register_file,
            descriptors: self.descriptors,
            cpu,
            error_buffer: self.error_buffer,
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
    error_buffer: Option<String>,
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

        (
            Arca {
                register_file: self.register_file,
                descriptors: self.descriptors,
                page_table,
                error_buffer: self.error_buffer,
            },
            self.cpu,
        )
    }

    pub fn swap(&mut self, other: &mut Arca) {
        core::mem::swap(&mut self.register_file, &mut other.register_file);
        core::mem::swap(&mut self.descriptors, &mut other.descriptors);
        core::mem::swap(&mut self.error_buffer, &mut other.error_buffer);
        self.cpu.swap_address_space(&mut other.page_table);
    }

    pub fn cpu(&mut self) -> &'_ mut Cpu {
        self.cpu
    }

    pub fn error_buffer(&self) -> Option<&String> {
        self.error_buffer.as_ref()
    }

    pub fn error_buffer_mut(&mut self) -> &mut String {
        self.error_buffer.get_or_insert_with(String::new)
    }

    pub fn reset_error(&mut self) {
        self.error_buffer = None;
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
