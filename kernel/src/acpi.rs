use core::sync::atomic::{AtomicPtr, Ordering};

use crate::vm;

#[repr(C, packed)]
#[derive(Debug)]
struct RootSystemDescriptionPointer {
    signature: [u8; 8],
    cksum: u8,
    oem_id: [u8; 6],
    revision: u8,
    rsdt_addr: u32,
    length: u32,
    xsdt_addr: usize,
    xcksum: u8,
    reserved: [u8; 3],
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct SystemDescriptionTable {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    cksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

impl SystemDescriptionTable {
    pub fn resolve(&self) -> Table {
        match self.signature {
            [b'X', b'S', b'D', b'T'] => Table::ExtendedSDT(ExtendedSystemDescriptionTable {
                header: self,
                current: unsafe { (self as *const SystemDescriptionTable).add(1) as *const usize },
            }),
            [b'A', b'P', b'I', b'C'] => Table::MultipleAPIC(MultipleApicDescriptionTable {
                header: self,
                current: unsafe {
                    (self as *const SystemDescriptionTable).add(1).byte_add(8) as *const (u8, u8)
                },
            }),
            _ => Table::Unknown,
        }
    }
}

#[derive(Debug)]
pub enum Table<'a> {
    ExtendedSDT(ExtendedSystemDescriptionTable<'a>),
    MultipleAPIC(MultipleApicDescriptionTable<'a>),
    Unknown,
}

#[derive(Debug, Copy, Clone)]
pub struct ExtendedSystemDescriptionTable<'a> {
    header: &'a SystemDescriptionTable,
    current: *const usize,
}

#[derive(Debug, Copy, Clone)]
pub struct MultipleApicDescriptionTable<'a> {
    header: &'a SystemDescriptionTable,
    current: *const (u8, u8),
}

#[allow(unused)]
#[derive(Debug)]
pub enum ApicDescription {
    Local(u8, u8, u32),
    Local2(u32, u32, u32),
    Unknown(u8),
}

static RSDP: AtomicPtr<RootSystemDescriptionPointer> = AtomicPtr::new(core::ptr::null_mut());

unsafe fn locate_rsdp() -> &'static RootSystemDescriptionPointer {
    let saved = RSDP.load(Ordering::SeqCst);
    if !saved.is_null() {
        return &*saved;
    }
    let mut addr = 0x000E0000;
    while addr < 0x000FFFFF {
        let ptr: *mut RootSystemDescriptionPointer = vm::pa2ka(addr);
        if (*ptr).signature == "RSD PTR ".as_bytes() {
            RSDP.store(ptr, Ordering::SeqCst);
            assert!((*ptr).revision == 2);
            return &*ptr;
        }
        addr += 16;
    }
    panic!("could not find rsdp");
}

pub unsafe fn get_xsdt() -> ExtendedSystemDescriptionTable<'static> {
    let rsdp = locate_rsdp();
    let xsdt: &'static SystemDescriptionTable = &*vm::pa2ka(rsdp.xsdt_addr);
    if let Table::ExtendedSDT(xsdt) = xsdt.resolve() {
        return xsdt;
    }
    unreachable!();
}

impl<'a> Iterator for ExtendedSystemDescriptionTable<'a> {
    type Item = Table<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let header = self.header as *const SystemDescriptionTable;
            let current = self.current;
            let end = header.byte_add(self.header.length as usize) as *const usize;
            if current >= end {
                return None;
            }
            let result: *const SystemDescriptionTable = vm::pa2ka(self.current.read_unaligned());
            let next = current.add(1);
            self.current = &*next;
            Some((*result).resolve())
        }
    }
}

impl<'a> Iterator for MultipleApicDescriptionTable<'a> {
    type Item = ApicDescription;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let header = self.header as *const SystemDescriptionTable;
            let current = self.current;
            let (ty, length) = current.read_unaligned();
            let result = match ty {
                0 => {
                    let current: *const (u8, u8, u8, u8, u32) = core::mem::transmute(current);
                    let (_, _, acpi_id, apic_id, flags) = current.read_unaligned();
                    ApicDescription::Local(acpi_id, apic_id, flags)
                }
                9 => {
                    let current: *const (u8, u8, u32, u32, u32) = core::mem::transmute(current);
                    let (_, _, apic_id, flags, acpi_id) = current.read_unaligned();
                    ApicDescription::Local2(apic_id, flags, acpi_id)
                }
                x => ApicDescription::Unknown(x),
            };
            let end = header.byte_add(self.header.length as usize) as *const (u8, u8);
            if current >= end {
                return None;
            }
            assert!(length != 0);
            let next = current.byte_add(length as usize);
            self.current = &*next;
            Some(result)
        }
    }
}
