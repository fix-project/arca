use core::{ffi::CStr, ops::Range};

use crate::vm;

#[repr(C)]
#[derive(Debug)]
pub struct MultibootInfo {
    flags: u32,
    mem_lower: u32,
    mem_upper: u32,
    boot_device: u32,
    cmdline: u32,
    mods_count: u32,
    mods_addr: u32,
    syms: [u32; 4],
    mmap_length: u32,
    mmap_addr: u32,
    drives_length: u32,
    drives_addr: u32,
    config_table: u32,
    boot_loader_name: u32,
    apm_table: u32,
    vbe: [u32; 6],
    framebuffer: [u32; 5],
    framebuffer_type: u8,
    color_info: [u8; 6],
}

#[derive(Debug, Eq, PartialEq)]
pub enum MappingType {
    Available,
    Acpi,
    NonVolatile,
    Defective,
    Reserved,
}

impl From<u32> for MappingType {
    fn from(value: u32) -> Self {
        match value {
            1 => MappingType::Available,
            3 => MappingType::Acpi,
            4 => MappingType::NonVolatile,
            5 => MappingType::Defective,
            _ => MappingType::Reserved,
        }
    }
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct MemoryMapping {
    size: u32,
    base: u64,
    length: u64,
    mapping_type: u32,
}

impl MemoryMapping {
    pub fn base(&self) -> usize {
        self.base as usize
    }

    pub fn len(&self) -> usize {
        self.length as usize
    }

    pub fn mapping_type(&self) -> MappingType {
        self.mapping_type.into()
    }

    pub fn available(&self) -> bool {
        self.mapping_type() == MappingType::Available
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MemoryMap<'a> {
    base: &'a MemoryMapping,
    length: usize,
    current: &'a MemoryMapping,
}

impl<'a> Iterator for MemoryMap<'a> {
    type Item = &'a MemoryMapping;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        let current_p = self.current as *const MemoryMapping;
        let base_p = self.base as *const MemoryMapping;
        let length = self.length;
        if unsafe { current_p >= base_p.byte_add(length) } {
            return None;
        }
        self.current = unsafe { &*current_p.byte_add(current.size as usize + 4) };
        Some(current)
    }
}

extern "C" {
    static _sbss: u8;
    static _ebss: u8;
}

impl MultibootInfo {
    pub fn cmdline(&self) -> Option<&CStr> {
        if ((self.flags >> 2) & 1) == 1 {
            Some(unsafe { CStr::from_ptr(vm::pa2ka(self.cmdline as usize)) })
        } else {
            None
        }
    }

    pub fn memory_map(&self) -> Option<MemoryMap> {
        if ((self.flags >> 6) & 1) == 1 {
            let base = unsafe { &*vm::pa2ka(self.mmap_addr as usize) };
            Some(MemoryMap {
                base,
                length: self.mmap_length as usize,
                current: base,
            })
        } else {
            None
        }
    }

    pub fn modules(&self) -> Option<Modules> {
        if ((self.flags >> 3) & 1) == 1 && self.mods_count != 0 {
            let base = unsafe { &*vm::pa2ka(self.mods_addr as usize) };
            Some(Modules {
                base,
                count: self.mods_count as usize,
                current: base,
            })
        } else {
            None
        }
    }
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct Module {
    start: u32,
    end: u32,
    string: u32,
    reserved: u32,
}

impl Module {
    pub fn label(&self) -> Option<&CStr> {
        if self.string == 0 {
            None
        } else {
            Some(unsafe { CStr::from_ptr(vm::pa2ka(self.string as usize)) })
        }
    }

    pub fn data(&self) -> &[u8] {
        unsafe {
            core::slice::from_ptr_range(Range {
                start: vm::pa2ka(self.start as usize),
                end: vm::pa2ka(self.end as usize),
            })
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Modules<'a> {
    base: &'a Module,
    count: usize,
    current: &'a Module,
}

impl<'a> Iterator for Modules<'a> {
    type Item = &'a Module;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;
        let current_p = self.current as *const Module;
        let base_p = self.base as *const Module;
        if unsafe { current_p >= base_p.add(self.count) } {
            return None;
        }
        self.current = unsafe { &*current_p.add(1) };
        Some(current)
    }
}
