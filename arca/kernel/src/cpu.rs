use core::{
    cell::RefCell,
    ops::{Index, IndexMut},
};

use alloc::format;

use crate::{initcell::LazyLock, prelude::*, vm::ka2pa};

use crate::types::table::Table;
use crate::xstate::XState;

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

/// Process register state, including the state that stays on the CPU across syscalls.
#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct CompleteRegisterFile {
    pub registers: RegisterFile,
    pub fsbase: u64,
    pub xstate: XState,
}

impl CompleteRegisterFile {
    pub(crate) fn save(&mut self) {
        self.xstate.save();
        unsafe {
            core::arch::asm!(
                "rdfsbase {base}",
                base = out(reg) self.fsbase,
                options(nostack, preserves_flags),
            );
        }
    }

    pub(crate) fn restore(&self) {
        self.xstate.restore();
        unsafe {
            core::arch::asm!(
                "wrfsbase {base}",
                base = in(reg) self.fsbase,
                options(nostack, preserves_flags),
            );
        }
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

#[derive(Debug, Eq, PartialEq)]
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

unsafe extern "C" {
    fn set_pt(page_map: usize);
    // fn flush_tlb() -> usize;
    fn syscall_call_user(registers: &mut RegisterFile) -> ExitStatus;
    fn isr_call_user(registers: &mut RegisterFile) -> ExitStatus;
}

impl Cpu {
    pub fn activate_address_space(&mut self, table: Table) {
        if self.pml4.entry(0).is_some_and(|entry| entry.present()) {
            drop(self.deactivate_address_space());
        }
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

    pub fn deactivate_address_space(&mut self) -> Table {
        let AugmentedUnmappedPage::UniqueTable(mut pdpt) = self.pml4.entry_mut(0).unmap() else {
            todo!();
        };
        if pdpt.len() != 1 || pdpt.offset() != 0 {
            return Table::Table512GB(pdpt.into());
        }
        let mut pd = match pdpt.unmap(0) {
            AugmentedUnmappedPage::UniqueTable(pd) => pd,
            entry => {
                pdpt.entry_mut(0).replace(entry);
                return Table::Table512GB(pdpt.into());
            }
        };
        self.pdpt = Some(pdpt);
        if pd.len() != 1 || pd.offset() != 0 {
            return Table::Table1GB(pd.into());
        }
        let pt = match pd.unmap(0) {
            AugmentedUnmappedPage::UniqueTable(pt) => pt,
            entry => {
                pd.entry_mut(0).replace(entry);
                return Table::Table1GB(pd.into());
            }
        };
        self.pd = Some(pd);
        Table::Table2MB(pt.into())
    }

    pub fn map(&mut self, address: usize, entry: Entry) -> Result<Entry, crate::types::Error> {
        let length = entry.len();
        if length == 0 {
            return Ok(entry);
        }
        if ![1 << 12, 1 << 21, 1 << 30].contains(&length)
            || !address.is_multiple_of(length)
            || address.checked_add(length).is_none_or(|end| end > 1 << 39)
        {
            return Err(crate::types::Error::MapError);
        }

        if !self.pml4.can_map(address, length) {
            return Err(crate::types::Error::MapError);
        }
        let relaxed = matches!(&entry, arca::Entry::RWPage(_) | arca::Entry::RWTable(_))
            && self.pml4.prepare_write_path(address, length);

        let i_512gb = (address >> 39) & 0x1ff;
        assert_eq!(i_512gb, 0);

        let pml4 = &mut self.pml4;
        let pdpt = match pml4.entry_mut(i_512gb).unmap() {
            AugmentedUnmappedPage::None => AugmentedPageTable::new(),
            AugmentedUnmappedPage::UniqueTable(pt) => pt,
            AugmentedUnmappedPage::SharedTable(pt)
            | AugmentedUnmappedPage::CopyOnWriteTable(pt) => SharedPage::into_unique(pt),
            _ => todo!(),
        };
        let table = Table::from(CowPage::Unique(pdpt));
        let mut table = arca::Table::from_inner(table);
        let result = table.map(address, entry).map_err(|e| match e {
            arca::table::MapError::Runtime(e) => e,
            arca::table::MapError::MapExists => crate::types::Error::MapError,
        });
        match table.into_inner() {
            Table::Table512GB(page) => pml4.entry_mut(i_512gb).chain_unique(page.unique()),
            _ => todo!(),
        };
        if relaxed {
            self.flush_user_translations();
        } else {
            self.invalidate(address, length);
        }
        result
    }

    fn invalidate(&mut self, address: usize, length: usize) {
        unsafe {
            if length == 4096 {
                core::arch::asm!("invlpg [{}]", in(reg) address);
            } else {
                self.flush_user_translations();
            }
        }
    }

    fn flush_user_translations(&mut self) {
        unsafe { set_pt(ka2pa(Box::as_ptr(&self.pml4))) };
    }

    pub(crate) fn replace_zero_range(
        &mut self,
        address: usize,
        length: usize,
        writable: bool,
    ) -> Result<usize, crate::types::Error> {
        if length == 0 {
            return Ok(0);
        }
        let length = length
            .checked_next_multiple_of(4096)
            .ok_or(crate::types::Error::MapError)?;
        let end = address
            .checked_add(length)
            .filter(|end| *end <= 1 << 39)
            .ok_or(crate::types::Error::MapError)?;
        if !address.is_multiple_of(4096) || !self.pml4.can_replace_zero_range(address..end) {
            return Err(crate::types::Error::MapError);
        }
        let page_table = ka2pa(Box::as_ptr(&self.pml4));
        let mut retired = common::arrayvec::ArrayVec::<crate::paging::RetiredLeaf, 512>::new();
        let changed = self
            .pml4
            .replace_zero_range(address..end, 0, writable, &mut |_, old| {
                if !matches!(old, AugmentedUnmappedPage::None) {
                    retired.push(old).unwrap();
                    if retired.is_full() {
                        // No user-memory access occurs during this walk.
                        unsafe { set_pt(page_table) };
                        retired.empty();
                    }
                }
            });
        if changed {
            self.flush_user_translations();
        } else if !retired.is_empty() {
            self.invalidate(address, length);
        }
        drop(retired);
        Ok(length)
    }

    pub(crate) fn resolve_cow(&mut self, address: usize) -> bool {
        if !crate::vm::is_user(address) {
            return false;
        }
        let root = ka2pa(Box::as_ptr(&self.pml4));
        self.pml4.resolve_cow(address, &mut |all| unsafe {
            if all {
                set_pt(root);
            } else {
                core::arch::asm!("invlpg [{}]", in(reg) address);
            }
        })
    }

    /// # Safety
    /// An appropriate page table must have been set before calling this function.
    pub unsafe fn run(&mut self, registers: &mut RegisterFile) -> ExitReason {
        loop {
            let syscall_safe = registers[Register::RCX] == registers.rip
                && registers[Register::R11] == registers.flags
                && registers.mode == Mode::User;
            let result: ExitReason = if syscall_safe {
                unsafe { syscall_call_user(registers).into() }
            } else {
                unsafe { isr_call_user(registers).into() }
            };
            if let ExitReason::PageFault { addr, error } = result
                && error == 7
                && self.resolve_cow(addr)
            {
                continue;
            }
            return result;
        }
    }
}

impl From<ExitReason> for Value {
    fn from(value: ExitReason) -> Self {
        let result = format!("{value:x?}");
        Value::Blob(Blob::from_inner(result.into()))
    }
}
