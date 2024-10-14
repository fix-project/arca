use core::{
    arch::{asm, x86_64::_mm_pause},
    cell::LazyCell,
    ops::Range,
    ptr::{addr_of, addr_of_mut},
    sync::atomic::{AtomicBool, AtomicPtr, Ordering},
};

use alloc::vec::Vec;
use log::LevelFilter;

use crate::{
    acpi::{self, ApicDescription, Table},
    buddy::{self, UniquePage2MB},
    debugcon::DebugLogger,
    gdt::{GdtDescriptor, PrivilegeLevel},
    idt::{GateType, Idt, IdtDescriptor, IdtEntry},
    msr,
    multiboot::MultibootInfo,
    page::UniquePage,
    paging::{self, PageTable, PageTable256TB, PageTable512GB, PageTableEntry, Permissions},
    refcnt::{self, SharedPage},
    registers::{ControlReg0, ControlReg4, ExtendedFeatureEnableReg},
    spinlock::SpinLock,
    vm,
};

extern "C" {
    fn kmain() -> !;
    fn set_gdt(gdtr: *const GdtDescriptor);
    static mut _sstack: u8;
    static mut _sbss: u8;
    static mut _ebss: u8;
    static mut _scdata: u8;
    static mut _lcdata: u8;
    static mut _ecdata: u8;
    static mut trampoline_start: u8;
    static mut trampoline_end: u8;
    static isr_table: [usize; 256];
}

#[no_mangle]
static NEXT_STACK_ADDR: AtomicPtr<u8> = AtomicPtr::new(core::ptr::null_mut());

#[no_mangle]
static NEXT_CPU_READY: AtomicBool = AtomicBool::new(false);

static LOGGER: DebugLogger = DebugLogger;

static mut IDT: LazyCell<Idt> = LazyCell::new(|| {
    Idt(unsafe {
        isr_table.map(|address| {
            IdtEntry::new(
                address,
                0x08,
                None,
                GateType::Interrupt,
                PrivilegeLevel::User,
            )
        })
    })
});

pub(crate) static KERNEL_PAGES: SpinLock<LazyCell<SharedPage<PageTable512GB>>> =
    SpinLock::new(LazyCell::new(|| unsafe {
        let mut pdpt = PageTable512GB::new();
        for (i, entry) in pdpt.iter_mut().enumerate() {
            entry.map_global(i << 30, Permissions::All);
        }
        pdpt.into()
    }));

pub(crate) static PAGE_MAP: SpinLock<LazyCell<SharedPage<PageTable256TB>>> =
    SpinLock::new(LazyCell::new(|| {
        let pdpt = KERNEL_PAGES.lock().clone();
        let mut map = PageTable256TB::new();
        map[256].chain(pdpt);
        map.into()
    }));

#[no_mangle]
unsafe extern "C" fn _rsstart_bsp(multiboot_pa: usize) -> *mut u8 {
    let multiboot = vm::pa2ka(multiboot_pa);
    init_bss();

    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Info);
    init_allocators(multiboot);

    let cpus: Vec<u32> = acpi::get_xsdt()
        .find_map(|table| {
            if let Table::MultipleAPIC(madt) = table {
                Some(madt)
            } else {
                None
            }
        })
        .expect("did not find madt")
        .filter_map(|x| match x {
            ApicDescription::Local(_, apic, _) => Some(apic as u32),
            ApicDescription::Local2(apic, _, _) => Some(apic),
            _ => None,
        })
        .collect();
    log::info!("found {} processors", cpus.len());

    LazyCell::force(&*addr_of!(IDT));
    LazyCell::force(&*PAGE_MAP.lock());

    init_cpu_config();
    init_cpu_tls();
    init_syscalls();
    crate::lapic::init(true);

    let mut lapic = crate::lapic::LAPIC.borrow_mut();
    *crate::cpuinfo::ACPI_ID = lapic.id();
    *crate::cpuinfo::IS_BOOTSTRAP = true;
    crate::cpuinfo::NCORES.store(cpus.len(), Ordering::SeqCst);

    let trampoline = core::slice::from_ptr_range(Range {
        start: addr_of!(trampoline_start),
        end: addr_of!(trampoline_end),
    });
    let target = core::slice::from_raw_parts_mut(0x8000 as *mut u8, trampoline.len());
    target.copy_from_slice(trampoline);
    let stack_page = UniquePage2MB::new().into_raw();
    let stack_addr = addr_of_mut!((*stack_page.add(1))[0]);
    for cpu in cpus {
        if cpu == lapic.id() {
            continue;
        }
        NEXT_CPU_READY.store(false, Ordering::SeqCst);
        let page = UniquePage2MB::new().into_raw();
        NEXT_STACK_ADDR.store(addr_of_mut!((*page.add(1))[0]), Ordering::SeqCst);
        log::debug!("Booting CPU {cpu} with stack {:p}", NEXT_STACK_ADDR);
        lapic.boot_cpu(cpu, 0x8000);
        while !NEXT_CPU_READY.load(Ordering::SeqCst) {
            _mm_pause();
        }
    }

    let gdtr = GdtDescriptor::new(&**crate::gdt::GDT);
    set_gdt(addr_of!(gdtr));
    asm!("ltr {tss:x}", tss=in(reg) 0x30);

    let idtr: IdtDescriptor = (&*IDT).into();
    asm!("lidt [{addr}]", addr=in(reg) addr_of!(idtr));

    stack_addr
}

#[no_mangle]
unsafe extern "C" fn _rsstart_ap() {
    init_cpu_config();
    init_cpu_tls();
    init_syscalls();
    crate::lapic::init(false);

    let lapic = crate::lapic::LAPIC.borrow();
    let id = lapic.id();
    *crate::cpuinfo::ACPI_ID = id;
    *crate::cpuinfo::IS_BOOTSTRAP = false;

    let gdtr = GdtDescriptor::new(&**crate::gdt::GDT);
    set_gdt(addr_of!(gdtr));
    asm!("ltr {tss:x}", tss=in(reg) 0x30);

    let idtr: IdtDescriptor = (&*IDT).into();
    asm!("lidt [{addr}]", addr=in(reg) addr_of!(idtr));

    log::debug!("CPU {id} ready!");
}

unsafe fn init_cpu_config() {
    crate::registers::write_cr0(ControlReg0::PE | ControlReg0::PG | ControlReg0::MP);
    crate::registers::write_cr4(ControlReg4::PAE | ControlReg4::PGE | ControlReg4::FSGSBASE);
    crate::registers::write_efer(
        ExtendedFeatureEnableReg::LME
            | ExtendedFeatureEnableReg::SCE
            | ExtendedFeatureEnableReg::NXE,
    );
}

#[no_mangle]
unsafe extern "C" fn _rscontinue() -> ! {
    crate::tsc::init();
    crate::kvmclock::init();

    let map = PAGE_MAP.lock().clone();
    crate::cpu::CPU.borrow_mut().activate_page_table(map);

    asm!("sti");
    kmain();
}

unsafe fn init_bss() {
    let start = addr_of_mut!(_sbss);
    let end = addr_of_mut!(_ebss);
    let length = end.offset_from(start) as usize;
    let bss = core::slice::from_raw_parts_mut(start, length);
    bss.fill(0);
}

unsafe fn init_allocators(multiboot: *const MultibootInfo) {
    let multiboot: &MultibootInfo = &*multiboot;
    log::info!(
        "kernel command line: {:?}",
        multiboot.cmdline().expect("could not find command line")
    );
    let mmap = multiboot
        .memory_map()
        .expect("could not get memory map from bootloader");
    let modules = multiboot.modules();
    if let Some(modules) = modules {
        for module in modules {
            log::info!(
                "Found module: {:?} @ {:p}",
                module.label(),
                module.data().as_ptr()
            );
        }
    }

    buddy::init(mmap);
    refcnt::init();
}

unsafe fn init_cpu_tls() {
    let start = addr_of!(_scdata) as usize;
    let end = addr_of!(_ecdata) as usize;
    let load = vm::pa2ka::<u8>(addr_of!(_lcdata) as usize);
    let length = end - start;

    let src = core::slice::from_raw_parts(load, length);
    let dst = src.to_vec();

    asm! {
        "wrgsbase {base}; mov gs:[0], {base}", base=in(reg) dst.as_ptr()
    }
    msr::wrmsr(0xC0000102, dst.as_ptr() as u64); // kernel GS base; actually really user GS base
    core::mem::forget(dst);
}

extern "C" {
    fn syscall_handler();
}

unsafe fn init_syscalls() {
    // p 175: https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/24593.pdf
    crate::msr::wrmsr(0xC0000081, ((0x18 | 0b11) << 48) | (0x08 << 32)); // STAR
    crate::msr::wrmsr(0xC0000082, syscall_handler as usize as u64); // LSTAR
    crate::msr::wrmsr(0xC0000083, syscall_handler as usize as u64); // CSTAR
    crate::msr::wrmsr(0xC0000084, 0); // SFMASK
}
