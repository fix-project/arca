use crate::{
    cpu::{Cpu, ExitStatus, RegisterFile},
    paging::{HardwarePageTable, PageTable256TB, PageTableEntry, Permissions},
    refcnt::RcPage,
};

pub struct Arca {
    page_table: RcPage<PageTable256TB>,
    register_file: RegisterFile,
}

impl Arca {
    pub fn new() -> Arca {
        let mut page_table = PageTable256TB::new();
        let mut pdpt = unsafe { crate::rsstart::KERNEL_PAGES.clone() };
        let kernel = pdpt.make_mut();
        kernel[0].protect(Permissions::All);
        page_table[256].chain(pdpt, Permissions::All);
        let register_file = RegisterFile::new();
        Arca {
            page_table: page_table.into(),
            register_file,
        }
    }

    pub fn load(self, cpu: &mut Cpu) -> LoadedArca<'_> {
        unsafe { cpu.activate_page_table(self.page_table) };
        LoadedArca {
            register_file: self.register_file,
            cpu,
        }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }
}

impl Default for Arca {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LoadedArca<'a> {
    register_file: RegisterFile,
    cpu: &'a mut Cpu,
}

impl LoadedArca<'_> {
    pub fn run(&mut self) -> ExitStatus {
        unsafe { self.cpu.run(&mut self.register_file) }
    }

    pub fn registers(&self) -> &RegisterFile {
        &self.register_file
    }

    pub fn registers_mut(&mut self) -> &mut RegisterFile {
        &mut self.register_file
    }

    pub fn unload(self) -> Arca {
        let page_table = unsafe {
            self.cpu
                .activate_page_table(crate::rsstart::PAGE_MAP.clone())
                .unwrap()
        };

        Arca {
            register_file: self.register_file,
            page_table,
        }
    }

    pub fn swap(&mut self, other: &mut Arca) {
        other.page_table = unsafe {
            self.cpu
                .activate_page_table(other.page_table.clone())
                .unwrap()
        };
        core::mem::swap(&mut self.register_file, &mut other.register_file);
    }
}
