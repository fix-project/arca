use core::{
    cell::RefCell,
    ops::{Index, IndexMut},
};

use alloc::boxed::Box;

use crate::{initcell::InitCell, prelude::*, types::pagetable::Entry, vm::ka2pa};

#[core_local]
pub static CPU: InitCell<RefCell<Cpu>> = InitCell::new(|| {
    let mut pml4 = AugmentedPageTable::new();
    pml4.entry_mut(256)
        .chain_shared(crate::rsstart::KERNEL_MAPPINGS.clone());
    RefCell::new(Cpu {
        size: None,
        offset: 0,
        pml4,
        pdpt: Some(AugmentedPageTable::new()),
        pd: Some(AugmentedPageTable::new()),
        pt: Some(AugmentedPageTable::new()),
    })
});

pub struct Cpu {
    size: Option<usize>,
    offset: usize,
    pml4: UniquePage<AugmentedPageTable<PageTable256TB>>,
    pdpt: Option<UniquePage<AugmentedPageTable<PageTable512GB>>>,
    pd: Option<UniquePage<AugmentedPageTable<PageTable1GB>>>,
    pt: Option<UniquePage<AugmentedPageTable<PageTable2MB>>>,
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
            AddressSpace::AddressSpace4KB(offset, entry) => {
                self.size = Some(12);
                self.offset = offset;

                let mut pt = self.pt.take().unwrap();
                let mut pd = self.pd.take().unwrap();
                let mut pdpt = self.pdpt.take().unwrap();

                match entry {
                    Entry::UniquePage(p) => pt.entry_mut(offset & 0x1ff).map_unique(p),
                    Entry::SharedPage(p) => pt.entry_mut(offset & 0x1ff).map_shared(p),
                    Entry::UniqueTable(t) => pt.entry_mut(offset & 0x1ff).chain_unique(t),
                    Entry::SharedTable(t) => pt.entry_mut(offset & 0x1ff).chain_shared(t),
                };

                pd.entry_mut((offset >> 9) & 0x1ff).chain_unique(pt);
                pdpt.entry_mut((offset >> 18) & 0x1ff).chain_unique(pd);
                self.pml4
                    .entry_mut((offset >> 27) & 0x1ff)
                    .chain_unique(pdpt);
            }
            AddressSpace::AddressSpace2MB(offset, entry) => {
                self.size = Some(21);
                self.offset = offset;

                let mut pd = self.pd.take().unwrap();
                let mut pdpt = self.pdpt.take().unwrap();

                match entry {
                    Entry::UniquePage(p) => pd.entry_mut(offset & 0x1ff).map_unique(p),
                    Entry::SharedPage(p) => pd.entry_mut(offset & 0x1ff).map_shared(p),
                    Entry::UniqueTable(t) => pd.entry_mut(offset & 0x1ff).chain_unique(t),
                    Entry::SharedTable(t) => pd.entry_mut(offset & 0x1ff).chain_shared(t),
                };

                pdpt.entry_mut((offset >> 9) & 0x1ff).chain_unique(pd);
                self.pml4
                    .entry_mut((offset >> 18) & 0x1ff)
                    .chain_unique(pdpt);
            }
            AddressSpace::AddressSpace1GB(offset, entry) => {
                self.size = Some(30);
                self.offset = offset;

                let mut pdpt = self.pdpt.take().unwrap();

                match entry {
                    Entry::UniquePage(p) => pdpt.entry_mut(offset & 0x1ff).map_unique(p),
                    Entry::SharedPage(p) => pdpt.entry_mut(offset & 0x1ff).map_shared(p),
                    Entry::UniqueTable(t) => pdpt.entry_mut(offset & 0x1ff).chain_unique(t),
                    Entry::SharedTable(t) => pdpt.entry_mut(offset & 0x1ff).chain_shared(t),
                };

                self.pml4
                    .entry_mut((offset >> 9) & 0x1ff)
                    .chain_unique(pdpt);
            }
        }
        unsafe {
            set_pt(ka2pa(Box::as_ptr(&self.pml4)));
        }
    }

    pub fn deactivate_address_space(&mut self) -> AddressSpace {
        match self.size.take() {
            Some(12) => {
                let offset = self.offset;
                let AugmentedUnmappedPage::UniqueTable(mut pdpt) =
                    self.pml4.entry_mut((offset >> 27) & 0x1ff).unmap()
                else {
                    panic!();
                };
                let AugmentedUnmappedPage::UniqueTable(mut pd) =
                    pdpt.entry_mut((offset >> 18) & 0x1ff).unmap()
                else {
                    panic!();
                };
                let AugmentedUnmappedPage::UniqueTable(mut pt) =
                    pd.entry_mut((offset >> 9) & 0x1ff).unmap()
                else {
                    panic!();
                };
                let entry = match pt.entry_mut(offset & 0x1ff).unmap() {
                    AugmentedUnmappedPage::UniquePage(p) => Entry::UniquePage(p),
                    AugmentedUnmappedPage::UniqueTable(t) => Entry::UniqueTable(t),
                    _ => todo!(),
                };
                self.pt = Some(pt);
                self.pd = Some(pd);
                self.pdpt = Some(pdpt);
                AddressSpace::AddressSpace4KB(offset, entry)
            }
            Some(21) => {
                let offset = self.offset;
                let AugmentedUnmappedPage::UniqueTable(mut pdpt) =
                    self.pml4.entry_mut((offset >> 18) & 0x1ff).unmap()
                else {
                    panic!();
                };
                let AugmentedUnmappedPage::UniqueTable(mut pd) =
                    pdpt.entry_mut((offset >> 9) & 0x1ff).unmap()
                else {
                    panic!();
                };
                let entry = match pd.entry_mut(offset & 0x1ff).unmap() {
                    AugmentedUnmappedPage::None => todo!(),
                    AugmentedUnmappedPage::UniquePage(_) => todo!(),
                    AugmentedUnmappedPage::SharedPage(_) => todo!(),
                    AugmentedUnmappedPage::Global(_) => todo!(),
                    AugmentedUnmappedPage::UniqueTable(t) => Entry::UniqueTable(t),
                    AugmentedUnmappedPage::SharedTable(t) => Entry::SharedTable(t),
                };
                self.pdpt = Some(pdpt);
                self.pd = Some(pd);
                AddressSpace::AddressSpace2MB(offset, entry)
            }
            Some(30) => {
                let offset = self.offset;
                let AugmentedUnmappedPage::UniqueTable(mut pdpt) =
                    self.pml4.entry_mut((offset >> 9) & 0x1ff).unmap()
                else {
                    panic!();
                };
                let AugmentedUnmappedPage::UniqueTable(pd) = pdpt.entry_mut(offset & 0x1ff).unmap()
                else {
                    panic!();
                };
                self.pdpt = Some(pdpt);
                AddressSpace::AddressSpace1GB(offset, Entry::UniqueTable(pd))
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
