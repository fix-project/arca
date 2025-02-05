use core::{
    cell::RefCell,
    ops::{Index, IndexMut},
};

use crate::{initcell::InitCell, prelude::*, types::pagetable::UniqueEntry, vm::ka2pa};

#[core_local]
pub static CPU: InitCell<RefCell<Cpu>> = InitCell::new(|| {
    let mut pml4 = PageTable256TB::new();
    pml4[256].chain_shared(crate::rsstart::KERNEL_MAPPINGS.clone());
    RefCell::new(Cpu {
        size: None,
        offset: 0,
        pml4,
        pdpt: Some(PageTable512GB::new()),
        pd: Some(PageTable1GB::new()),
        pt: Some(PageTable2MB::new()),
    })
});

pub struct Cpu {
    size: Option<usize>,
    offset: usize,
    pml4: UniquePage<PageTable256TB>,
    pdpt: Option<UniquePage<PageTable512GB>>,
    pd: Option<UniquePage<PageTable1GB>>,
    pt: Option<UniquePage<PageTable2MB>>,
}

impl !Sync for Cpu {}

#[repr(u64)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Mode {
    Supervisor,
    User,
}

#[repr(C)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RegisterFile {
    registers: [u64; 16],
    rip: u64,
    flags: u64,
    mode: Mode,
}

impl RegisterFile {
    pub fn new() -> RegisterFile {
        RegisterFile {
            registers: [0; 16],
            rip: 0,
            flags: 0x202,
            mode: Mode::User,
        }
    }
}

impl Default for RegisterFile {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Register {
    RAX,
    RCX,
    RDX,
    RBX,
    RSP,
    RBP,
    RSI,
    RDI,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    RIP,
    RFLAGS,
}

impl Index<Register> for RegisterFile {
    type Output = u64;

    fn index(&self, index: Register) -> &Self::Output {
        match index {
            Register::RIP => &self.rip,
            Register::RFLAGS => &self.flags,
            x => &self.registers[x as usize],
        }
    }
}

impl IndexMut<Register> for RegisterFile {
    fn index_mut(&mut self, index: Register) -> &mut Self::Output {
        match index {
            Register::RIP => &mut self.rip,
            Register::RFLAGS => &mut self.flags,
            x => &mut self.registers[x as usize],
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ExitStatus {
    pub code: u64,
    pub error: u64,
}

extern "C" {
    fn set_pt(page_map: usize);
    fn syscall_call_user(registers: &mut RegisterFile) -> ExitStatus;
    fn isr_call_user(registers: &mut RegisterFile) -> ExitStatus;
}

impl Cpu {
    pub fn activate_address_space(&mut self, address_space: AddressSpace) {
        match address_space {
            AddressSpace::AddressSpace0B => {}
            AddressSpace::AddressSpace4KB(offset, unique_entry) => {
                self.size = Some(12);
                self.offset = offset;
                match unique_entry {
                    UniqueEntry::Page(p) => {
                        self.pt.as_mut().unwrap()[(offset >> 12) & 0x1ff].map_unique(p)
                    }
                    UniqueEntry::Table(t) => {
                        self.pt.as_mut().unwrap()[(offset >> 12) & 0x1ff].chain_unique(t)
                    }
                };
                self.pd.as_mut().unwrap()[(offset >> 21) & 0x1ff]
                    .chain_unique(self.pt.take().unwrap());
                self.pdpt.as_mut().unwrap()[(offset >> 30) & 0x1ff]
                    .chain_unique(self.pd.take().unwrap());
                self.pml4[(offset >> 39) & 0x1ff].chain_unique(self.pdpt.take().unwrap());
            }
            AddressSpace::AddressSpace2MB(offset, unique_entry) => {
                self.size = Some(21);
                self.offset = offset;
                match unique_entry {
                    UniqueEntry::Page(p) => {
                        self.pd.as_mut().unwrap()[(offset >> 21) & 0x1ff].map_unique(p)
                    }
                    UniqueEntry::Table(t) => {
                        self.pd.as_mut().unwrap()[(offset >> 21) & 0x1ff].chain_unique(t)
                    }
                };
                self.pdpt.as_mut().unwrap()[(offset >> 30) & 0x1ff]
                    .chain_unique(self.pd.take().unwrap());
                self.pml4[(offset >> 39) & 0x1ff].chain_unique(self.pdpt.take().unwrap());
            }
        }
        unsafe {
            set_pt(ka2pa(self.pml4.as_ptr()));
        }
    }

    pub fn deactivate_address_space(&mut self) -> AddressSpace {
        match self.size.take() {
            Some(21) => {
                let offset = self.offset;
                let HardwareUnmappedPage::UniqueTable(mut pdpt) =
                    self.pml4[(offset >> 39) & 0x1ff].unmap()
                else {
                    panic!();
                };
                let HardwareUnmappedPage::UniqueTable(mut pd) =
                    pdpt[(offset >> 30) & 0x1ff].unmap()
                else {
                    panic!();
                };
                let HardwareUnmappedPage::UniqueTable(pt) = pd[(offset >> 21) & 0x1ff].unmap()
                else {
                    todo!();
                };
                self.pdpt = Some(pdpt);
                self.pd = Some(pd);
                AddressSpace::AddressSpace2MB(offset, UniqueEntry::Table(pt))
            }
            None => AddressSpace::new(),
            _ => todo!(),
        }
    }

    /// # Safety
    /// An appropriate page table must have been set before calling this function.
    pub unsafe fn run(&mut self, registers: &mut RegisterFile) -> ExitStatus {
        let syscall_safe = registers[Register::RCX] == registers.rip
            && registers[Register::R11] == registers.flags
            && registers.mode == Mode::User;
        if syscall_safe {
            unsafe { syscall_call_user(registers) }
        } else {
            unsafe { isr_call_user(registers) }
        }
    }
}
