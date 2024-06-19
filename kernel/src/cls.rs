use core::{arch::asm, ops::Deref};

use alloc::boxed::Box;

use crate::{
    buddy::Page2MB,
    gdt::{GdtEntry, Readability, Writeability},
    idt::PrivilegeLevel,
    kvmclock::CpuTimeInfo,
    tss::TaskStateSegment,
};

#[repr(C)]
pub struct CoreLocalData {
    pub gs_base: usize,
    pub interrupt_stack_top: *mut u8,
    pub saved_rsp: usize,
    pub bootstrap: bool,
    pub acpi_id: usize,
    pub tss: TaskStateSegment,
    pub gdt: [GdtEntry; 8],
    pub tsc_frequency_mhz: u32,
    pub cpu_time_info: *mut CpuTimeInfo,
}

impl CoreLocalData {
    pub fn new(bootstrap: bool, acpi_id: usize) -> Box<Self> {
        let interrupt_stack = Page2MB::new().expect("could not allocate thread stack");
        let tss = TaskStateSegment::new(&interrupt_stack);
        let interrupt_stack_top = unsafe { interrupt_stack.into_raw().add(Page2MB::LENGTH) };
        let gdt = [
            GdtEntry::null(),                                                // 00: null
            GdtEntry::code64(Readability::Readable, PrivilegeLevel::System), // 08: kernel code
            GdtEntry::data(Writeability::Writeable, PrivilegeLevel::System), // 10: kernel data
            GdtEntry::null(), // 18: user code (32-bit)
            GdtEntry::data(Writeability::Writeable, PrivilegeLevel::User), // 20: user data
            GdtEntry::code64(Readability::Readable, PrivilegeLevel::User), // 28: user code (64-bit)
            GdtEntry::tss0(&tss), // 30: TSS (low)
            GdtEntry::tss1(&tss), // 38: TSS (high)
        ];
        let cpu_time_info = Box::into_raw(Default::default());
        let mut data = Box::new(Self {
            gs_base: 0,
            interrupt_stack_top,
            saved_rsp: 0,
            bootstrap,
            acpi_id,
            tss,
            gdt,
            tsc_frequency_mhz: 0,
            cpu_time_info,
        });
        data.gs_base = data.as_ref() as *const CoreLocalData as usize;
        data
    }
}

pub struct CoreLocalStorage {}

pub static CLS: CoreLocalStorage = CoreLocalStorage {};

impl Deref for CoreLocalStorage {
    type Target = CoreLocalData;

    fn deref(&self) -> &Self::Target {
        unsafe {
            let mut gs_base: *const CoreLocalData;
            asm!(
                "mov {gs_base}, gs:[0]", gs_base=out(reg)gs_base
            );
            &*gs_base
        }
    }
}

impl CoreLocalStorage {
    pub fn with_mut<T>(&self, f: impl FnOnce(&mut CoreLocalData) -> T) -> T {
        unsafe {
            let mut gs_base: *mut CoreLocalData;
            asm!(
                "mov {gs_base}, gs:[0]", gs_base=out(reg)gs_base
            );
            f(&mut *gs_base)
        }
    }
}
