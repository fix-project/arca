use core::ffi::CStr;

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
    pub fn base(&self) -> *const u8 {
        self.base as *const u8
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
        if current.base().is_null() && current.len() == 0 {
            return None;
        }
        if unsafe { current_p >= base_p.byte_add(length) } {
            return None;
        }
        self.current = unsafe { &*current_p.byte_add(current.size as usize + 4) };
        Some(current)
    }
}

impl MultibootInfo {
    pub fn cmdline(&self) -> Option<&CStr> {
        if ((self.flags >> 2) & 1) == 1 {
            Some(unsafe { CStr::from_ptr(vm::pa2ka(self.cmdline as *const i8)) })
        } else {
            None
        }
    }

    pub fn memory_map(&self) -> Option<MemoryMap> {
        if ((self.flags >> 6) & 1) == 1 {
            let base = unsafe { &*vm::pa2ka(self.mmap_addr as *const MemoryMapping) };
            Some(MemoryMap {
                base,
                length: self.mmap_length as usize,
                current: base,
            })
        } else {
            None
        }
    }
}
