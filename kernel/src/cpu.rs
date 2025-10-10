use core::{
    cell::RefCell,
    ops::{Index, IndexMut},
};

use alloc::format;

use crate::{initcell::LazyLock, prelude::*, vm::ka2pa};

use crate::types::table::Table;

#[core_local]
pub static CPU: LazyLock<RefCell<Cpu>> = LazyLock::new(|| {
    let mut pml4 = AugmentedPageTable::new();
    let mappings = crate::rsstart::KERNEL_MAPPINGS.clone();
    unsafe {
        let mappings = core::mem::transmute::<
            *mut AugmentedPageTable<PageTable512GB>,
            *const PageTable512GB,
        >(SharedPage::into_raw(mappings));
        pml4.entry_mut(256)
            .chain_unchecked(mappings, crate::paging::Permissions::All);
    }
    RefCell::new(Cpu {
        pml4,
        pdpt: None,
        pd: None,
    })
});

#[derive(Debug)]
pub struct Cpu {
    pml4: UniquePage<AugmentedPageTable<PageTable256TB>>,
    pdpt: Option<UniquePage<AugmentedPageTable<PageTable512GB>>>,
    pd: Option<UniquePage<AugmentedPageTable<PageTable1GB>>>,
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
    registers: [u64; 16], // 0x0
    rip: u64,             // 0x80
    flags: u64,           // 0x88
    mode: Mode,           // 0x90
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

impl Index<usize> for RegisterFile {
    type Output = u64;

    fn index(&self, index: usize) -> &Self::Output {
        if index < 16 {
            &self.registers[index]
        } else if index == 16 {
            &self[Register::RIP]
        } else if index == 17 {
            &self[Register::RFLAGS]
        } else {
            panic!("invalid register");
        }
    }
}

impl IndexMut<usize> for RegisterFile {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index < 16 {
            &mut self.registers[index]
        } else if index == 16 {
            &mut self[Register::RIP]
        } else if index == 17 {
            &mut self[Register::RFLAGS]
        } else {
            panic!("invalid register");
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ExitStatus {
    pub code: u64,
    pub error: u64,
}

#[derive(Debug)]
pub enum ExitReason {
    DivisionByZero,
    Debug,
    Interrupted(usize),
    Breakpoint,
    InvalidInstruction,
    DeviceNotAvailable,
    DoubleFault,
    InvalidTSS { selector: usize },
    SegmentNotPresent { selector: usize },
    StackSegmentFault { selector: usize },
    GeneralProtectionFault { selector: usize },
    PageFault { addr: usize, error: u64 },
    FloatingPointException,
    AlignmentCheck { code: u64 },
    MachineCheck,
    SIMDException,
    VirtualizationException,
    ControlProtectionException,
    HypervisorInjectionException,
    VMMCommunicationException,
    SecurityException,
    SystemCall,
}

impl From<ExitStatus> for ExitReason {
    fn from(value: ExitStatus) -> Self {
        match value.code {
            0 => ExitReason::DivisionByZero,
            1 => ExitReason::Debug,
            3 => ExitReason::Breakpoint,
            6 => ExitReason::InvalidInstruction,
            7 => ExitReason::DeviceNotAvailable,
            8 => ExitReason::DoubleFault,
            10 => ExitReason::InvalidTSS {
                selector: value.error as usize,
            },
            11 => ExitReason::SegmentNotPresent {
                selector: value.error as usize,
            },
            12 => ExitReason::StackSegmentFault {
                selector: value.error as usize,
            },
            13 => ExitReason::GeneralProtectionFault {
                selector: value.error as usize,
            },
            14 => ExitReason::PageFault {
                addr: crate::registers::read_cr2() as usize,
                error: value.error,
            },
            16 => ExitReason::FloatingPointException,
            17 => ExitReason::AlignmentCheck { code: value.error },
            18 => ExitReason::MachineCheck,
            19 => ExitReason::SIMDException,
            20 => ExitReason::VirtualizationException,
            21 => ExitReason::ControlProtectionException,
            28 => ExitReason::HypervisorInjectionException,
            29 => ExitReason::VMMCommunicationException,
            30 => ExitReason::SecurityException,
            256 => ExitReason::SystemCall,
            x => ExitReason::Interrupted(x as usize),
        }
    }
}

extern "C" {
    fn set_pt(page_map: usize);
    // fn flush_tlb() -> usize;
    fn syscall_call_user(registers: &mut RegisterFile) -> ExitStatus;
    fn isr_call_user(registers: &mut RegisterFile) -> ExitStatus;
}

impl Cpu {
    pub fn activate_address_space(&mut self, table: Table) {
        match table {
            Table::Table2MB(table) => {
                let mut pd = self.pd.take().unwrap_or_else(AugmentedPageTable::new);
                let mut pdpt = self.pdpt.take().unwrap_or_else(AugmentedPageTable::new);

                pd.entry_mut(0).chain_unique(table.unique());
                pdpt.entry_mut(0).chain_unique(pd);
                self.pml4.entry_mut(0).chain_unique(pdpt);
            }
            Table::Table1GB(table) => {
                let mut pdpt = self.pdpt.take().unwrap_or_else(AugmentedPageTable::new);

                pdpt.entry_mut(0).chain_unique(table.unique());
                self.pml4.entry_mut(0).chain_unique(pdpt);
            }
            Table::Table512GB(table) => {
                self.pml4.entry_mut(0).chain_unique(table.unique());
            }
        }
        unsafe {
            set_pt(ka2pa(Box::as_ptr(&self.pml4)));
        }
    }

    pub fn swap_address_space(&mut self, new: &mut Table) {
        // TODO: unnecessary TLB invalidation
        unsafe {
            let replacement = core::ptr::read(new);
            core::ptr::write(new, self.deactivate_address_space());
            self.activate_address_space(replacement);
        }
    }

    pub fn deactivate_address_space(&mut self) -> Table {
        let AugmentedUnmappedPage::UniqueTable(mut pdpt) = self.pml4.unmap(0) else {
            todo!();
        };
        if pdpt.len() > 1 {
            return Table::Table512GB(pdpt.into());
        }
        let offset = pdpt.offset();
        let AugmentedUnmappedPage::UniqueTable(mut pd) = pdpt.unmap(offset) else {
            todo!();
        };
        debug_assert!(pdpt.is_empty());
        debug_assert!(self.pdpt.is_none());
        self.pdpt = Some(pdpt);
        if pd.len() > 1 {
            return Table::Table1GB(pd.into());
        }
        let offset = pd.offset();
        let AugmentedUnmappedPage::UniqueTable(pt) = pd.unmap(offset) else {
            todo!();
        };
        debug_assert!(pd.is_empty());
        debug_assert!(self.pd.is_none());
        self.pd = Some(pd);
        if pt.len() > 1 {
            return Table::Table2MB(pt.into());
        }
        Table::default()
    }

    pub fn map(&mut self, address: usize, entry: Entry) -> Result<Entry, crate::types::Error> {
        assert!(crate::vm::is_user(address));

        let i_512gb = (address >> 39) & 0x1ff;
        assert_eq!(i_512gb, 0);

        let pml4 = &mut self.pml4;
        // TODO: check behavior when inserting unique into shared
        let pdpt = match pml4.entry_mut(i_512gb).unmap() {
            AugmentedUnmappedPage::None => {
                todo!("inserting into larger-than-1GB address space");
                // AugmentedPageTable::new()
            }
            AugmentedUnmappedPage::UniqueTable(pt) => pt,
            AugmentedUnmappedPage::SharedTable(pt) => RefCnt::into_unique(pt),
            _ => todo!(),
        };
        let table = Table::from(CowPage::Unique(pdpt));
        let mut table = arca::Table::from_inner(table);
        let result = table.map(address, entry)?;
        match table.into_inner() {
            Table::Table512GB(page) => pml4.entry_mut(i_512gb).chain_unique(page.unique()),
            _ => todo!(),
        };
        unsafe {
            core::arch::asm!("invlpg [{pg}]", pg=in(reg)address);
        }
        Ok(result)
    }

    /// # Safety
    /// An appropriate page table must have been set before calling this function.
    pub unsafe fn run(&mut self, registers: &mut RegisterFile) -> ExitReason {
        let syscall_safe = registers[Register::RCX] == registers.rip
            && registers[Register::R11] == registers.flags
            && registers.mode == Mode::User;
        if syscall_safe {
            unsafe { syscall_call_user(registers).into() }
        } else {
            unsafe { isr_call_user(registers).into() }
        }
    }
}

impl From<ExitReason> for Value {
    fn from(value: ExitReason) -> Self {
        let result = format!("{value:x?}");
        Value::Blob(Blob::from_inner(result.into()))
    }
}
