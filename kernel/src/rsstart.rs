use core::{
    arch::asm,
    ptr::addr_of_mut,
    sync::atomic::{AtomicBool, Ordering},
};

use log::LevelFilter;

use crate::{
    buddy::{self, Page2MB},
    debugcon::DebugLogger,
    multiboot::MultibootInfo,
    vm,
};

extern "C" {
    fn kmain() -> !;
    static mut _sbss: u8;
    static mut _ebss: u8;
    static mut _stdata: u8;
    static mut _ltdata: u8;
    static mut _etdata: u8;
    static mut _stbss: u8;
    static mut _etbss: u8;
}

static LOGGER: DebugLogger = DebugLogger;
static SEMAPHORE: AtomicBool = AtomicBool::new(true);

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
    } else {
        while SEMAPHORE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            core::arch::x86_64::_mm_pause();
        }
    }
    init_cpu_tls(id, bsp, ncores);

    let stack_top = init_cpu_stack(id);

    SEMAPHORE.store(false, Ordering::SeqCst);
    stack_top
}

#[no_mangle]
unsafe extern "C" fn _rscontinue() -> ! {
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
