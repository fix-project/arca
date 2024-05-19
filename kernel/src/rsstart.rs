use core::{
    arch::asm,
    ptr::addr_of_mut,
    sync::atomic::{AtomicBool, Ordering},
};

use log::LevelFilter;

use crate::{
    buddy::{self, Page2MB, Page4KB},
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
        let start = addr_of_mut!(_sbss);
        let end = addr_of_mut!(_ebss);
        let length = end.offset_from(start) as usize;
        start.write_bytes(0, length);

        let _ = log::set_logger(&LOGGER);
        log::set_max_level(LevelFilter::Info);

        let multiboot: &MultibootInfo = &*multiboot;
        log::info!("{:#x?}", multiboot.cmdline());
        let mmap = multiboot
            .memory_map()
            .expect("could not get memory map from bootloader");

        buddy::init(mmap);
    } else {
        while SEMAPHORE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            core::arch::x86_64::_mm_pause();
        }
    }

    {
        let stdata = addr_of_mut!(_stdata);
        let ltdata = addr_of_mut!(_ltdata);
        let etdata = addr_of_mut!(_etdata);
        let stbss = addr_of_mut!(_stbss);
        let etbss = addr_of_mut!(_etbss);

        let ntdata = etdata.offset_from(stdata) as usize;
        let ntbss = etbss.offset_from(stbss) as usize;

        let total = ntdata + ntbss;

        let alloc_size = total.next_power_of_two();
        assert!(alloc_size < 4096);
        let mut tls = Page4KB::new().expect("could not allocate TLS");
        tls.kernel().write_bytes(0, 4096);
        tls.kernel()
            .copy_from_nonoverlapping(vm::pa2ka(ltdata), ntdata);

        let starttls = addr_of_mut!(tls[0]);
        let starttls: *mut u64 = core::mem::transmute(starttls);
        starttls.write_volatile(tls.kernel() as u64);

        log::debug!("CPU {} is using {:p}+4KB as TLS", id, tls.physical());

        asm! {
            "wrfsbase {base}", base=in(reg) tls.kernel()
        }
        core::mem::forget(tls);
    }

    crate::CPU_ACPI_ID = id;
    crate::CPU_IS_BOOTSTRAP = bsp;
    crate::CPU_NCORES = ncores;

    let stack = Page2MB::new().expect("could not allocate stack");
    log::debug!("CPU {} is using {:p}+2MB as %rsp", id, stack.physical());

    let stack_bottom = stack.kernel();
    let stack_top = stack_bottom.add(0x200000);
    core::mem::forget(stack);
    SEMAPHORE.store(false, Ordering::SeqCst);
    stack_top
}

#[no_mangle]
unsafe extern "C" fn _rscontinue() -> ! {
    kmain();
}
