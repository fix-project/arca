use core::{
    cell::RefCell,
    ops::{Index, IndexMut},
};

use alloc::format;

use crate::{initcell::LazyLock, prelude::*, types::pagetable::Entry, vm::ka2pa};

#[core_local]
pub static CPU: LazyLock<RefCell<Cpu>> = LazyLock::new(|| {
    let mut pml4 = AugmentedPageTable::new();
    pml4.entry_mut(256)
        .chain_shared(crate::rsstart::KERNEL_MAPPINGS.clone());
    RefCell::new(Cpu {
        size: None,
        offset: 0,
        pml4,
        pdpt: None,
        pd: None,
        pt: None,
    })
});

#[derive(Debug)]
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
    pub fn activate_address_space(&mut self, address_space: AddressSpace) {
        match address_space {
            AddressSpace::AddressSpace0B => {}
            AddressSpace::AddressSpace4KB(offset, entry) => {
                self.size = Some(12);
                self.offset = offset;

                let mut pt = self.pt.take().unwrap_or_else(AugmentedPageTable::new);
                let mut pd = self.pd.take().unwrap_or_else(AugmentedPageTable::new);
                let mut pdpt = self.pdpt.take().unwrap_or_else(AugmentedPageTable::new);

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

                let mut pd = self.pd.take().unwrap_or_else(|| AugmentedPageTable::new());
                let mut pdpt = self
                    .pdpt
                    .take()
                    .unwrap_or_else(|| AugmentedPageTable::new());

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

                let mut pdpt = self
                    .pdpt
                    .take()
                    .unwrap_or_else(|| AugmentedPageTable::new());

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
            // crate::tlb::shootdown();
            crate::tlb::clear_pending();
            crate::tlb::set_sleeping(false);
        }
    }

    pub fn swap_address_space(&mut self, new: &mut AddressSpace) {
        // TODO: unnecessary TLB invalidation
        unsafe {
            let replacement = core::ptr::read(new);
            core::ptr::write(new, self.deactivate_address_space());
            self.activate_address_space(replacement);
        }
    }

    pub fn deactivate_address_space(&mut self) -> AddressSpace {
        unsafe {
            crate::tlb::set_sleeping(true);
            crate::tlb::clear_pending();
        }
        let AugmentedUnmappedPage::UniqueTable(mut pdpt) = self.pml4.unmap(0) else {
            todo!();
        };
        if pdpt.len() > 1 {
            todo!("pdpt with {} entries", pdpt.len());
        }
        let offset = pdpt.offset();
        let AugmentedUnmappedPage::UniqueTable(mut pd) = pdpt.unmap(offset) else {
            todo!();
        };
        debug_assert!(pdpt.is_empty());
        debug_assert!(self.pdpt.is_none());
        self.pdpt = Some(pdpt);
        if pd.len() > 1 {
            return AddressSpace::AddressSpace1GB(offset, Entry::UniqueTable(pd));
        }
        let offset = pd.offset();
        let AugmentedUnmappedPage::UniqueTable(pt) = pd.unmap(offset) else {
            todo!();
        };
        debug_assert!(pd.is_empty());
        debug_assert!(self.pd.is_none());
        self.pd = Some(pd);
        if pt.len() > 1 {
            return AddressSpace::AddressSpace2MB(offset, Entry::UniqueTable(pt));
        }
        todo!();
    }

    pub fn map_unique_4kb(&mut self, address: usize, page: UniquePage<Page4KB>) {
        assert!(crate::vm::is_user(address));
        let i_512gb = (address >> 39) & 0x1ff;
        assert_eq!(i_512gb, 0);
        let i_1gb = (address >> 30) & 0x1ff;
        let i_2mb = (address >> 21) & 0x1ff;
        let i_4kb = (address >> 12) & 0x1ff;

        let pml4 = &mut self.pml4;
        // TODO: check behavior when inserting unique into shared
        let mut pdpt = match pml4.entry_mut(i_512gb).unmap() {
            AugmentedUnmappedPage::None => {
                todo!("inserting into larger-than-1GB address space");
                // AugmentedPageTable::new()
            }
            AugmentedUnmappedPage::UniqueTable(pt) => pt,
            AugmentedUnmappedPage::SharedTable(pt) => RefCnt::into_unique(pt),
            _ => todo!(),
        };
        let mut pd = match pdpt.entry_mut(i_1gb).unmap() {
            AugmentedUnmappedPage::None => {
                todo!("inserting into larger-than-1GB address space");
                // AugmentedPageTable::new()
            }
            AugmentedUnmappedPage::UniqueTable(pt) => pt,
            AugmentedUnmappedPage::SharedTable(pt) => RefCnt::into_unique(pt),
            _ => todo!(),
        };
        let mut pt = match pd.entry_mut(i_2mb).unmap() {
            AugmentedUnmappedPage::None => AugmentedPageTable::new(),
            AugmentedUnmappedPage::UniqueTable(pt) => pt,
            AugmentedUnmappedPage::SharedTable(pt) => RefCnt::into_unique(pt),
            _ => todo!(),
        };
        pt.entry_mut(i_4kb).map_unique(page);
        pd.entry_mut(i_2mb).chain_unique(pt);
        pdpt.entry_mut(i_1gb).chain_unique(pd);
        pml4.entry_mut(i_512gb).chain_unique(pdpt);
        unsafe {
            core::arch::asm!("invlpg [{pg}]", pg=in(reg)address);
            crate::tlb::shootdown();
        }
    }

    /// # Safety
    /// An appropriate page table must have been set before calling this function.
    pub unsafe fn run(&mut self, registers: &mut RegisterFile) -> ExitReason {
        assert_eq!(registers.flags & 0x200, 0x200);
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
        let result = format!("{:x?}", value);
        Value::Atom(result)
    }
}
