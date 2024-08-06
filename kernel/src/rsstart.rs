use core::{
    arch::asm,
    cell::LazyCell,
    ptr::{addr_of, addr_of_mut},
    sync::atomic::{AtomicBool, Ordering},
};

use log::LevelFilter;

use crate::{
    buddy::{self, Block, Page2MB},
    debugcon::DebugLogger,
    gdt::{GdtDescriptor, PrivilegeLevel},
    idt::{GateType, Idt, IdtDescriptor, IdtEntry},
    msr,
    multiboot::MultibootInfo,
    paging::{self, PageTable256TB, PageTable512GB, PageTableEntry, Permissions},
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
    static isr_table: [usize; 256];
}

static LOGGER: DebugLogger = DebugLogger;
static SEMAPHORE: AtomicBool = AtomicBool::new(true);

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

#[no_mangle]
unsafe extern "C" fn _rsstart(id: u32, bsp: bool, ncores: u32, multiboot_pa: usize) -> *mut u8 {
    asm!("cli");
    if bsp {
        let multiboot = vm::pa2ka(multiboot_pa);
        init_bss();

        let _ = log::set_logger(&LOGGER);
        log::set_max_level(LevelFilter::Info);
        init_buddy_allocator(multiboot);

        LazyCell::force(&*addr_of!(IDT));
        crate::cpuinfo::NCORES.store(ncores as usize, Ordering::Release);
    } else {
        while SEMAPHORE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            core::arch::x86_64::_mm_pause();
        }
    }
    init_cpu_tls();
    *crate::cpuinfo::ACPI_ID = id as usize;
    *crate::cpuinfo::IS_BOOTSTRAP = bsp;
    init_syscalls();

    let gdtr = GdtDescriptor::new(&**crate::gdt::GDT);
    set_gdt(addr_of!(gdtr));
    asm!("ltr {tss:x}", tss=in(reg) 0x30);

    // enable new page table
    let mut pdpt = PageTable512GB::new();
    for (i, entry) in pdpt.iter_mut().enumerate() {
        entry.map(
            Block::<30>::from_raw((i << 30) as *mut u8),
            Permissions::All,
        );
    }

    let mut map = PageTable256TB::new();
    map[256].chain(pdpt, Permissions::All);

    paging::set_page_table(map);

    let stack_top = init_cpu_stack(id);

    SEMAPHORE.store(false, Ordering::SeqCst);
    stack_top
}

#[no_mangle]
unsafe extern "C" fn _rscontinue() -> ! {
    let idtr: IdtDescriptor = (&*IDT).into();
    asm!("lidt [{addr}]", addr=in(reg) addr_of!(idtr));

    crate::tsc::init();
    crate::kvmclock::init();
    crate::lapic::init();

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

unsafe fn init_buddy_allocator(multiboot: *const MultibootInfo) {
    let multiboot: &MultibootInfo = &*multiboot;
    log::info!(
        "kernel command line: {:?}",
        multiboot.cmdline().expect("could not find command line")
    );
    let mmap = multiboot
        .memory_map()
        .expect("could not get memory map from bootloader");

    buddy::init(mmap);
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

unsafe fn init_cpu_stack(id: u32) -> *mut u8 {
    let stack = Page2MB::new().expect("could not allocate stack");
    log::debug!("CPU {} is using {:p}+2MB as %rsp", id, stack.kernel());

    let stack_bottom = stack.kernel();
    let stack_top = stack_bottom.add(0x200000);
    core::mem::forget(stack);
    stack_top
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
