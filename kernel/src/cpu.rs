use core::{
    cell::RefCell,
    ops::{Index, IndexMut},
};

use crate::{page::SharedPage, paging::PageTable256TB, vm::ka2pa};

#[core_local]
pub static CPU: RefCell<Cpu> = RefCell::new(Cpu {
    current_page_table: None,
});

pub struct Cpu {
    current_page_table: Option<SharedPage<PageTable256TB>>,
}

impl !Sync for Cpu {}

#[repr(u64)]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Mode {
    Supervisor,
    User,
}

#[repr(C)]
#[derive(Debug, Clone)]
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
    // pub fn id() -> u32 {
    //     crate::cpuinfo::id()
    // }

    // pub fn bootstrap() -> bool {
    //     crate::cpuinfo::is_bootstrap()
    // }

    /// # Safety
    /// This page table must not cause any violations of Rust's memory safety rules.  This
    /// generally means kernel data structures should not be mapped in any other location.
    pub unsafe fn activate_page_table(
        &mut self,
        page_table: SharedPage<PageTable256TB>,
    ) -> Option<SharedPage<PageTable256TB>> {
        set_pt(ka2pa(page_table.as_ptr()));
        self.current_page_table.replace(page_table)
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

    /// # Safety
    /// This function may trigger undefined behavior if the modifications being made would affect
    /// the currently running code.
    pub unsafe fn modify_page_table(&mut self, f: impl FnOnce(&mut SharedPage<PageTable256TB>)) {
        let mut pt = None;
        core::mem::swap(&mut self.current_page_table, &mut pt);
        let mut pt = pt.expect("cannot modify nonexistent page table");
        f(&mut pt);
        set_pt(ka2pa(pt.as_ptr()));
        self.current_page_table = Some(pt);
    }
}
