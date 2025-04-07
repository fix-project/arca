use core::mem::ManuallyDrop;

use alloc::vec::Vec;
use common::util::spinlock::SpinLockGuard;

use crate::{cpu::ExitReason, prelude::*};

use super::Value;

static PT_LOCK: SpinLock<()> = SpinLock::new(());

pub fn take_pt_lock(lock: &SpinLock<()>) -> Option<SpinLockGuard<()>> {
    if crate::is_serialized() {
        Some(lock.lock())
    } else {
        None
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Arca {
    valid: bool,
    page_table: ManuallyDrop<AddressSpace>,
    register_file: ManuallyDrop<RegisterFile>,
    descriptors: ManuallyDrop<Vec<Value>>,
}

impl Arca {
    pub fn new() -> Arca {
        let page_table = AddressSpace::new();
        let register_file = RegisterFile::new();

        let lock = take_pt_lock(&PT_LOCK);
        let arca = Arca {
            valid: true,
            page_table: ManuallyDrop::new(page_table),
            register_file: ManuallyDrop::new(register_file),
            descriptors: ManuallyDrop::new(Vec::with_capacity(1024)),
        };
        core::mem::drop(lock);
        arca
    }

    pub fn load(mut self, cpu: &mut Cpu) -> LoadedArca<'_> {
        unsafe {
            self.valid = false;
            let register_file = ManuallyDrop::take(&mut self.register_file);
            let descriptors = ManuallyDrop::take(&mut self.descriptors);
            let page_table = ManuallyDrop::take(&mut self.page_table);
            cpu.activate_address_space(page_table);
            LoadedArca {
                valid: true,
                register_file: ManuallyDrop::new(register_file),
                descriptors: ManuallyDrop::new(descriptors),
                cpu: ManuallyDrop::new(cpu),
            }
        }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn mappings(&self) -> &AddressSpace {
        &self.page_table
    }

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

impl Clone for Arca {
    fn clone(&self) -> Self {
        assert!(self.valid);
        let lock = take_pt_lock(&PT_LOCK);
        let page_table = self.page_table.clone();
        core::mem::drop(lock);
        let arca = Arca {
            valid: true,
            page_table,
            register_file: self.register_file.clone(),
            descriptors: self.descriptors.clone(),
        };
        unsafe {
            crate::tlb::shootdown();
        }
        arca
    }
}

impl Drop for Arca {
    fn drop(&mut self) {
        if self.valid {
            unsafe {
                let lock = take_pt_lock(&PT_LOCK);
                ManuallyDrop::drop(&mut self.page_table);
                core::mem::drop(lock);
                ManuallyDrop::drop(&mut self.register_file);
                ManuallyDrop::drop(&mut self.descriptors);
                crate::tlb::shootdown();
            }
        }
    }
}

#[derive(Debug)]
pub struct LoadedArca<'a> {
    valid: bool,
    register_file: ManuallyDrop<RegisterFile>,
    descriptors: ManuallyDrop<Vec<Value>>,
    cpu: ManuallyDrop<&'a mut Cpu>,
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

    pub fn descriptors(&self) -> &Vec<Value> {
        &self.descriptors
    }

    pub fn descriptors_mut(&mut self) -> &mut Vec<Value> {
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

    pub fn unload_with_cpu(mut self) -> (Arca, &'a mut Cpu) {
        unsafe {
            self.valid = false;
            let register_file = ManuallyDrop::take(&mut self.register_file);
            let descriptors = ManuallyDrop::take(&mut self.descriptors);
            let cpu = ManuallyDrop::take(&mut self.cpu);
            let page_table = cpu.deactivate_address_space();

            (
                Arca {
                    valid: true,
                    register_file: ManuallyDrop::new(register_file),
                    descriptors: ManuallyDrop::new(descriptors),
                    page_table: ManuallyDrop::new(page_table),
                },
                cpu,
            )
        }
    }

    pub fn swap(&mut self, other: &mut Arca) {
        core::mem::swap(&mut self.register_file, &mut other.register_file);
        core::mem::swap(&mut self.descriptors, &mut other.descriptors);
        self.cpu.swap_address_space(&mut other.page_table);
    }

    pub fn cpu(&mut self) -> &'_ mut Cpu {
        *self.cpu
    }
}

impl Drop for LoadedArca<'_> {
    fn drop(&mut self) {
        if self.valid {
            todo!("dropping loaded arca");
            // self.cpu.deactivate_address_space();
            // unsafe {
            //     ManuallyDrop::drop(&mut self.register_file);
            //     ManuallyDrop::drop(&mut self.descriptors);
            //     crate::tlb::shootdown();
            // }
        }
    }
}
