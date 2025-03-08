use core::{
    arch::asm,
    ptr::{addr_of, addr_of_mut},
};

use alloc::boxed::Box;
use log::LevelFilter;

use crate::{
    client::MESSENGER,
    debugcon::DEBUG,
    gdt::{GdtDescriptor, PrivilegeLevel},
    host::HOST,
    idt::{GateType, Idt, IdtDescriptor, IdtEntry},
    msr,
    paging::Permissions,
    prelude::*,
    spinlock::SpinLock,
    vm,
};

use common::message::Messenger;
use common::ringbuffer::{RingBufferEndPoint, RingBufferEndPointRawData};

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
    fn set_pt(page_map: usize);
}

static IDT: LazyLock<Idt> = LazyLock::new(|| {
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

pub(crate) static KERNEL_MAPPINGS: LazyLock<SharedPage<AugmentedPageTable<PageTable512GB>>> =
    LazyLock::new(|| unsafe {
        let mut pdpt = AugmentedPageTable::new();
        for i in 0..512 {
            pdpt.entry_mut(i).map_global(i << 30, Permissions::None);
        }
        pdpt.into()
    });

static SYNC: SpinLock<()> = SpinLock::new(());

#[no_mangle]
unsafe extern "C" fn _start(
    inner_offset: usize,
    inner_size: usize,
    refcnt_offset: usize,
    refcnt_size: usize,
    ring_buffer_data_ptr: usize,
) -> ! {
    let mut id = 0;
    core::arch::x86_64::__rdtscp(&mut id);
    let id = id as usize;

    let sync = if id == 0 {
        // one-time init
        init_bss();
        if cfg!(feature = "debugcon") {
            let _ = log::set_logger(&DEBUG);
        } else {
            let _ = log::set_logger(&HOST);
        }
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
        let raw = common::buddy::BuddyAllocatorRawData {
            base: vm::pa2ka(0),
            inner_offset,
            inner_size,
            refcnt_offset,
            refcnt_size,
        };
        let allocator = common::BuddyAllocator::from_raw_parts(raw);

        let sync = SYNC.lock();
        PHYSICAL_ALLOCATOR.set(allocator).unwrap();

        let raw_rb_data = Box::from_raw(
            PHYSICAL_ALLOCATOR.from_offset::<RingBufferEndPointRawData>(ring_buffer_data_ptr)
                as *mut RingBufferEndPointRawData,
        );
        let endpoint = RingBufferEndPoint::from_raw_parts(&raw_rb_data, &PHYSICAL_ALLOCATOR);
        core::mem::forget(raw_rb_data);
        let messenger = Messenger::new(endpoint);
        let _ = MESSENGER.set(SpinLock::new(messenger));
        sync
    } else {
        PHYSICAL_ALLOCATOR.wait();
        SYNC.lock()
    };
    core::mem::drop(sync);

    // PHYSICAL_ALLOCATOR.wait();

    // per-cpu init
    init_cpu_tls();
    crate::kvmclock::init();
    crate::profile::init();

    let gdtr = GdtDescriptor::new(&**crate::gdt::GDT);
    set_gdt(addr_of!(gdtr));
    asm!("ltr {tss:x}", tss=in(reg) 0x30);

    let idtr: IdtDescriptor = (&*IDT).into();
    asm!("lidt [{addr}]", addr=in(reg) addr_of!(idtr));

    init_syscalls();

    crate::lapic::init();

    let mut pml4: UniquePage<AugmentedPageTable<PageTable256TB>> = AugmentedPageTable::new();
    pml4.entry_mut(256)
        .chain_shared(crate::rsstart::KERNEL_MAPPINGS.clone());
    set_pt(vm::ka2pa(Box::leak(pml4)));

    core::arch::asm!("sti");

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

unsafe fn init_cpu_tls() {
    let start = addr_of!(_scdata) as usize;
    let end = addr_of!(_ecdata) as usize;
    let load = vm::pa2ka::<u8>(addr_of!(_lcdata) as usize);
    let length = end - start;

    let allocator = PHYSICAL_ALLOCATOR.wait();
    let src = core::slice::from_raw_parts(load, length);
    let dst: &mut [u8] = Box::leak(src.to_vec_in(&allocator).into_boxed_slice());

    asm! {
        "wrgsbase {base}; mov gs:[0], {base}", base=in(reg) dst.as_mut_ptr()
    }
    msr::wrmsr(0xC0000102, dst.as_mut_ptr() as u64); // kernel GS base; actually really user GS base
}

unsafe fn init_syscalls() {
    // p 175: https://www.amd.com/content/dam/amd/en/documents/processor-tech-docs/programmer-references/24593.pdf
    crate::msr::wrmsr(0xC0000081, ((0x18 | 0b11) << 48) | (0x08 << 32)); // STAR
    crate::msr::wrmsr(0xC0000082, syscall_handler as usize as u64); // LSTAR
    crate::msr::wrmsr(0xC0000083, syscall_handler as usize as u64); // CSTAR
    crate::msr::wrmsr(0xC0000084, 0x200); // SFMASK
}
