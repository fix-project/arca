use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use arcane::SyscallError;

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

use crate::types::internal;

mod xstate;

use xstate::XSaveArea;
pub use xstate::XSaveData;

const STATE_MASK: u64 = 0x7;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Arca {
    page_table: Table,
    register_file: Box<RegisterFile>,
    descriptors: Descriptors,
    fsbase: u64,
    xsave: Box<XSaveArea>,
}

impl Arca {
    pub fn new() -> Arca {
        let page_table = Table::from_inner(internal::Table::default());
        let register_file = RegisterFile::new().into();
        let descriptors = Descriptors::new();
        let xsave = XSaveArea::default().into();

        Arca {
            page_table,
            register_file,
            descriptors,
            fsbase: 0,
            xsave,
        }
    }

    pub fn new_with(
        register_file: impl Into<Box<RegisterFile>>,
        page_table: Table,
        descriptors: Tuple,
        fsbase: impl Into<u64>,
        xsave: impl Into<Box<XSaveArea>>,
    ) -> Arca {
        let descriptors = Vec::from(descriptors.into_inner().into_inner()).into();

        Arca {
            page_table,
            register_file: register_file.into(),
            descriptors,
            fsbase: fsbase.into(),
            xsave: xsave.into(),
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        cpu.activate_address_space(self.page_table.into_inner());
        unsafe {
            core::arch::asm! {
                "wrfsbase {base}",
                base = in(reg) self.fsbase,
                options(nostack, preserves_flags)
            };

            core::arch::x86_64::_xrstor(self.xsave.as_ptr(), STATE_MASK);
        }

        LoadedArca {
            register_file: self.register_file,
            descriptors: self.descriptors,
            cpu,
            xsave: self.xsave,
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

    pub fn read(self) -> (RegisterFile, Table, Tuple, Word, XSaveData) {
        (
            *self.register_file,
            self.page_table,
            Tuple::from_inner(internal::Tuple::new(Vec::from(self.descriptors))),
            self.fsbase.into(),
            (*self.xsave).into(),
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
    xsave: Box<XSaveArea>,
}

impl<'a> LoadedArca<'a> {
    pub fn run(&mut self) -> ExitReason {
        unsafe { self.cpu.run(&mut self.register_file) }
    }

    pub fn single_step(&mut self) -> ExitReason {
        self.register_file[Register::RFLAGS] |= 0x100;
        let result = self.run();
        self.register_file[Register::RFLAGS] &= !0x100;
        result
    }

    pub fn single_step_with(&mut self, mut f: impl FnMut(&mut Self)) -> ExitReason {
        loop {
            let step = self.single_step();
            if step == ExitReason::Debug {
                f(self);
            } else {
                return step;
            }
        }
    }

    pub fn trace(&mut self) -> ExitReason {
        self.single_step_with(|this| {
            log::info!(
                "@{:#x} -> {:#x?}",
                this.register_file[Register::RIP],
                this.register_file
            );
        })
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

    pub fn unload(self) -> Arca {
        self.unload_with_cpu().0
    }

    pub fn take(&mut self) -> Arca {
        let mut original = Default::default();
        self.swap(&mut original);
        original
    }

    pub fn unload_with_cpu(self) -> (Arca, &'a mut Cpu) {
        // add SIMD support here?
        let page_table = Table::from_inner(self.cpu.deactivate_address_space());
        let mut fsbase: u64;
        let mut xsave = self.xsave;

        unsafe {
            core::arch::asm! {
                "rdfsbase {base}",
                base = out(reg) fsbase,
                options(nostack, preserves_flags)
            };

            core::arch::x86_64::_xsave(xsave.as_mut_ptr(), STATE_MASK);
        }

        (
            Arca {
                register_file: self.register_file,
                descriptors: self.descriptors,
                page_table,
                fsbase,
                xsave,
            },
            self.cpu,
        )
    }

    pub fn swap(&mut self, other: &mut Arca) {
        core::mem::swap(&mut self.register_file, &mut other.register_file);
        core::mem::swap(&mut self.descriptors, &mut other.descriptors);
        let mut fsbase: u64;
        unsafe {
            core::arch::asm!(
                "rdfsbase {old}; wrfsbase {new}",
                old = out(reg) fsbase,
                new = in(reg) other.fsbase,
                options(nostack, preserves_flags)
            );

            core::arch::x86_64::_xsave(self.xsave.as_mut_ptr(), STATE_MASK);
            core::arch::x86_64::_xrstor(other.xsave.as_ptr(), STATE_MASK);
            core::mem::swap(&mut self.xsave, &mut other.xsave);
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
        Ok(self.arca.descriptors.insert(value))
    }

    pub fn take(self, index: usize) -> core::result::Result<Value, SyscallError> {
        let value = self.arca.descriptors.take(index)?;
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
        let old = self.arca.cpu.map(address, entry)?;
        Ok(old)
    }
}
