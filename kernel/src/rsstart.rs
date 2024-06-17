use core::{
    arch::asm,
    cell::LazyCell,
    ptr::{addr_of, addr_of_mut},
    sync::atomic::{AtomicBool, Ordering},
};

use log::LevelFilter;

use crate::{
    buddy::{self, Page2MB},
    debugcon::DebugLogger,
    gdt::{GdtDescriptor, GdtEntry, PrivilegeLevel, Readability, Writeability},
    idt::{GateType, Idt, IdtDescriptor, IdtEntry},
    multiboot::MultibootInfo,
    tss::TaskStateSegment,
    vm,
};

extern "C" {
    fn kmain() -> !;
    fn set_gdt(gdtr: *const GdtDescriptor);
    static mut _sstack: u8;
    static mut _sbss: u8;
    static mut _ebss: u8;
    static mut _stdata: u8;
    static mut _ltdata: u8;
    static mut _etdata: u8;
    static mut _stbss: u8;
    static mut _etbss: u8;
    static isr_table: [usize; 256];
}

static LOGGER: DebugLogger = DebugLogger;
static SEMAPHORE: AtomicBool = AtomicBool::new(true);

#[thread_local]
static INTERRUPT_STACK: LazyCell<Page2MB> =
    LazyCell::new(|| Page2MB::new().expect("could not allocate thread stack"));

#[thread_local]
static TSS: LazyCell<TaskStateSegment> = LazyCell::new(|| TaskStateSegment::new(&INTERRUPT_STACK));

#[thread_local]
static mut GDT: LazyCell<[GdtEntry; 8]> = LazyCell::new(|| {
    [
        GdtEntry::null(),                                                // 00: null
        GdtEntry::code64(Readability::Readable, PrivilegeLevel::System), // 08: kernel code
        GdtEntry::data(Writeability::Writeable, PrivilegeLevel::System), // 10: kernel data
        GdtEntry::null(),                                                // 18: user code (32-bit)
        GdtEntry::data(Writeability::Writeable, PrivilegeLevel::User),   // 20: user data
        GdtEntry::code64(Readability::Readable, PrivilegeLevel::User),   // 28: user code (64-bit)
        GdtEntry::tss0(&*TSS),                                           // 30: TSS (low)
        GdtEntry::tss1(&*TSS),                                           // 38: TSS (high)
    ]
});

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
unsafe extern "C" fn _rsstart(
    id: u32,
    bsp: bool,
    ncores: u32,
    multiboot: *const MultibootInfo,
) -> *mut u8 {
    if bsp {
        init_bss();
        init_logging();
        init_buddy_allocator(multiboot);
        LazyCell::force(&*addr_of!(IDT));
    } else {
        while SEMAPHORE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            core::arch::x86_64::_mm_pause();
        }
    }
    init_cpu_tls(id, bsp, ncores);
    init_syscalls();

    let gdtr = GdtDescriptor::new(&*GDT);
    set_gdt(addr_of!(gdtr));
    asm!("ltr {tss:x}", tss=in(reg) 0x30);

    let stack_top = init_cpu_stack(id);

    SEMAPHORE.store(false, Ordering::SeqCst);
    stack_top
}

#[no_mangle]
unsafe extern "C" fn _rscontinue() -> ! {
    // since we're now running on the main stack, we can repurpose the initial 16KB stacks to store
    // data for interrupt handling
    asm!("cli");
    let idtr: IdtDescriptor = (&*IDT).into();
    asm!("lidt [{addr}]", addr=in(reg) addr_of!(idtr));
    crate::tsc::init();
    crate::kvmclock::init();
    init_cpu_data();
    // TODO: set up TSS
    // TODO: set up GDT
    // TODO: set up IDT
    kmain();
}

unsafe fn init_bss() {
    let start = addr_of_mut!(_sbss);
    let end = addr_of_mut!(_ebss);
    let length = end.offset_from(start) as usize;
    let bss = core::slice::from_raw_parts_mut(start, length);
    bss.fill(0);
}

fn init_logging() {
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Info);
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

unsafe fn init_cpu_tls(id: u32, bsp: bool, ncores: u32) {
    let stdata = addr_of_mut!(_stdata);
    let etdata = addr_of_mut!(_etdata);
    let stbss = addr_of_mut!(_stbss);
    let etbss = addr_of_mut!(_etbss);

    let ntdata = etdata.offset_from(stdata) as usize;
    let ntbss = etbss.offset_from(stbss) as usize;

    let total = ntdata + ntbss;
    let extra = core::mem::size_of::<u64>();

    let tdata_template = core::slice::from_raw_parts(vm::pa2ka(stdata), ntdata);

    let mut tls = alloc::vec![0x0; total + extra].into_boxed_slice();
    tls.fill(0);
    tls[..ntdata].copy_from_slice(tdata_template);

    let tp = addr_of_mut!(tls[0]).add(total);
    let tp: *mut u64 = core::mem::transmute(tp);
    *tp = tp as u64;

    log::debug!(
        "CPU {} is using {:p}+{:#x} as TLS",
        id,
        tls.as_ptr(),
        tls.len()
    );

    asm! {
        "wrfsbase {base}", base=in(reg) tp
    }
    core::mem::forget(tls);

    crate::CPU_ACPI_ID = id as usize;
    crate::CPU_IS_BOOTSTRAP = bsp;
    crate::CPU_NCORES = ncores as usize;
}

unsafe fn init_cpu_stack(id: u32) -> *mut u8 {
    let stack = Page2MB::new().expect("could not allocate stack");
    log::debug!("CPU {} is using {:p}+2MB as %rsp", id, stack.physical());

    let stack_bottom = stack.kernel();
    let stack_top = stack_bottom.add(0x200000);
    core::mem::forget(stack);
    stack_top
}

unsafe fn init_cpu_data() {
    let cpu_data_region = addr_of_mut!(_sstack).add(0x4000 * crate::CPU_ACPI_ID);
    log::debug!(
        "CPU {} is using {:p} for task switch data",
        crate::CPU_ACPI_ID,
        cpu_data_region
    );
    *(cpu_data_region as *mut *mut u8) = cpu_data_region;
    asm! {
        "wrgsbase {base}", base=in(reg) cpu_data_region
    }
}

extern "C" {
    fn syscall_handler_asm();
}

unsafe fn init_syscalls() {
    // p 175: https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/24593.pdf
    crate::msr::wrmsr(0xC0000081, ((0x18 | 0b11) << 48) | (0x08 << 32)); // STAR
    crate::msr::wrmsr(0xC0000082, syscall_handler_asm as usize as u64); // LSTAR
    crate::msr::wrmsr(0xC0000083, syscall_handler_asm as usize as u64); // CSTAR
    crate::msr::wrmsr(0xC0000084, 0); // SFMASK
}

#[no_mangle]
unsafe extern "C" fn isr_handler(code: u64) {
    if code < 32 {
        log::error!("got ISR {code:#x}");
        crate::halt();
    }
    log::info!("got ISR {code:#x}");
}
