use core::{
    arch::asm,
    cell::LazyCell,
    ptr::{addr_of, addr_of_mut},
};

use log::LevelFilter;

use crate::{
    gdt::{GdtDescriptor, PrivilegeLevel},
    host::HOST,
    idt::{GateType, Idt, IdtDescriptor, IdtEntry},
    initcell::InitCell,
    msr,
    paging::Permissions,
    prelude::*,
    vm,
};

extern "C" {
    fn kmain();
    fn set_gdt(gdtr: *const GdtDescriptor);
    static mut _sstack: u8;
    static mut _sbss: u8;
    static mut _ebss: u8;
    static mut _scdata: u8;
    static mut _lcdata: u8;
    static mut _ecdata: u8;
    static isr_table: [usize; 256];
    fn syscall_handler();
}

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

pub(crate) static KERNEL_MAPPINGS: InitCell<SharedPage<AugmentedPageTable<PageTable512GB>>> =
    InitCell::new(|| unsafe {
        let mut pdpt = AugmentedPageTable::new();
        for i in 0..512 {
            pdpt.entry_mut(i).map_global(i << 30, Permissions::None);
        }
        pdpt.into()
    });

#[no_mangle]
unsafe extern "C" fn _start(
    inner_offset: usize,
    inner_size: usize,
    refcnt_offset: usize,
    refcnt_size: usize,
) -> ! {
    init_bss();
    let _ = log::set_logger(&HOST);
    if cfg!(feature = "klog-trace") {
        log::set_max_level(LevelFilter::Trace);
    } else if cfg!(feature = "klog-debug") {
        log::set_max_level(LevelFilter::Debug);
    } else if cfg!(feature = "klog-info") {
        log::set_max_level(LevelFilter::Info);
    } else if cfg!(feature = "klog-warn") {
        log::set_max_level(LevelFilter::Warn);
    } else if cfg!(feature = "klog-error") {
        log::set_max_level(LevelFilter::Error);
    } else if cfg!(feature = "klog-off") {
        log::set_max_level(LevelFilter::Off);
    } else {
        log::set_max_level(LevelFilter::Info);
    }
    let allocator = common::BuddyAllocator::from_raw_parts(common::buddy::BuddyAllocatorRawData {
        base: vm::pa2ka(0),
        inner_offset,
        inner_size,
        refcnt_offset,
        refcnt_size,
    });
    InitCell::initialize(&PHYSICAL_ALLOCATOR, || allocator);

    init_cpu_tls();

    let gdtr = GdtDescriptor::new(&**crate::gdt::GDT);
    set_gdt(addr_of!(gdtr));
    asm!("ltr {tss:x}", tss=in(reg) 0x30);

    let idtr: IdtDescriptor = (&*IDT).into();
    asm!("lidt [{addr}]", addr=in(reg) addr_of!(idtr));

    init_syscalls();

    crate::kvmclock::init();

    crate::lapic::init();

    // asm!("sti");
    // loop {
    //     core::hint::spin_loop();
    // }
    kmain();
    crate::shutdown();
}

unsafe fn init_bss() {
    let start = addr_of_mut!(_sbss);
    let end = addr_of_mut!(_ebss);
    let length = end.offset_from(start) as usize;
    let bss = core::slice::from_raw_parts_mut(start, length);
    bss.fill(0);
}

#[core_local]
#[no_mangle]
#[used]
pub static FOO: usize = 0xcafeb0ba;

#[inline(never)]
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

unsafe fn init_syscalls() {
    // p 175: https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/24593.pdf
    crate::msr::wrmsr(0xC0000081, ((0x18 | 0b11) << 48) | (0x08 << 32)); // STAR
    crate::msr::wrmsr(0xC0000082, syscall_handler as usize as u64); // LSTAR
    crate::msr::wrmsr(0xC0000083, syscall_handler as usize as u64); // CSTAR
    crate::msr::wrmsr(0xC0000084, 0x200); // SFMASK
}
