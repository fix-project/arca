#![feature(allocator_api)]
#![feature(str_from_raw_parts)]

use std::collections::HashMap;
use std::process::ExitCode;

extern crate alloc;
use alloc::sync::Arc;
use std::mem;
use std::thread;

use common::message::Messenger;
use common::ringbuffer;
use common::BuddyAllocator;
use elf::endian::AnyEndian;
use elf::segment::ProgramHeader;
use elf::ElfBytes;
use kvm_bindings::KVM_MAX_CPUID_ENTRIES;
use kvm_ioctls::Kvm;
use kvm_ioctls::VcpuExit;
use std::cell::RefCell;
use vmm::client;

const ADD_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_USER_add"));
const KERNEL_ELF: &[u8] = include_bytes!(env!("CARGO_BIN_FILE_KERNEL_kernel"));

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    use std::ptr::null_mut;

    use kvm_bindings::kvm_userspace_memory_region;
    use kvm_bindings::KVM_MEM_LOG_DIRTY_PAGES;

    let mem_size = 0x100000000; // 4 GiB

    let kernel_elf = KERNEL_ELF;

    let kvm = Kvm::new().unwrap();
    let vm = kvm.create_vm().unwrap();
    vm.create_irq_chip().unwrap();

    let load_addr: *mut u8 = unsafe {
        libc::mmap(
            null_mut(),
            mem_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
            -1,
            0,
        ) as *mut u8
    };

    assert!(!load_addr.is_null());

    let slice = unsafe { core::slice::from_raw_parts_mut(load_addr, mem_size) };

    let allocator = BuddyAllocator::new(slice);

    let elf =
        ElfBytes::<AnyEndian>::minimal_parse(kernel_elf).expect("could not read kernel elf file");
    let start_address = elf.ehdr.e_entry;

    let segments: Vec<ProgramHeader> = elf
        .segments()
        .expect("could not find ELF segments")
        .iter()
        .collect();

    let align_down = |alignment: usize, value: usize| value & !(alignment - 1);

    let mut reserved = HashMap::new();
    let mut reserve = |addr: usize| -> *mut () {
        let addr = align_down(4096, addr);
        *reserved
            .entry(addr)
            .or_insert_with(|| allocator.reserve_raw(addr, 4096))
    };
    let mut reserve_range = |start: usize, end: usize| -> *mut [u8] {
        let mut current = (start / 4096) * 4096;
        let len = end - start;
        let result = reserve(current).wrapping_byte_add(start - current);
        current += 4096;
        while current < end {
            reserve(current);
            current += 4096;
        }
        core::ptr::slice_from_raw_parts_mut(result as *mut u8, len)
    };

    // let mut mapped = HashSet::new();
    for segment in segments {
        match segment.p_type {
            elf::abi::SHT_NULL => {}
            elf::abi::SHT_PROGBITS => {
                let start = segment.p_paddr as usize & !0xFFFF800000000000;
                let sz = segment.p_memsz as usize;
                let end = start + sz;
                let ptr = reserve_range(start, end);
                let offset = segment.p_offset as usize;
                let source = &kernel_elf[offset..offset + sz];
                unsafe {
                    (*ptr).copy_from_slice(source);
                }
            }
            0x60000000.. => {
                // OS-specific (probably GNU/Linux cruft)
            }
            x => unimplemented!("segment type {x:#x}"),
        }
    }

    let mem_region = kvm_userspace_memory_region {
        slot: 0,
        guest_phys_addr: 0,
        memory_size: mem_size as u64,
        userspace_addr: load_addr as u64,
        flags: KVM_MEM_LOG_DIRTY_PAGES,
    };
    unsafe { vm.set_user_memory_region(mem_region).unwrap() };

    let mut vcpu_fd = vm.create_vcpu(0).unwrap();

    // set up the CPU in long mode
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();

    let mut pdpt: Box<[u64; 0x200], &BuddyAllocator> =
        unsafe { Box::new_zeroed_in(&allocator).assume_init() };
    for (i, entry) in pdpt.iter_mut().enumerate() {
        *entry = ((i << 30) | 0x83) as u64;
    }

    let mut pml4: Box<[u64; 0x200], &BuddyAllocator> =
        unsafe { Box::new_zeroed_in(&allocator).assume_init() };
    // pml4[0] = allocator.to_offset(&*pdpt) as u64 | 3;
    pml4[256] = allocator.to_offset(&*pdpt) as u64 | 3;
    Box::leak(pdpt);

    vcpu_sregs.idt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };
    use common::controlreg::*;
    vcpu_sregs.cr0 |= ControlReg0::PG | ControlReg0::PE | ControlReg0::MP;
    vcpu_sregs.cr0 &= !ControlReg0::EM;
    vcpu_sregs.cr3 = allocator.to_offset(&*pml4) as u64;
    vcpu_sregs.cr4 = ControlReg4::PAE
        | ControlReg4::PGE
        | ControlReg4::OSFXSR
        | ControlReg4::OSXMMEXCPT
        | ControlReg4::FSGSBASE
        | ControlReg4::SMAP
        | ControlReg4::SMEP;
    vcpu_sregs.efer |= ExtendedFeatureEnableReg::LME
        | ExtendedFeatureEnableReg::LMA
        | ExtendedFeatureEnableReg::SCE;
    Box::leak(pml4);

    vcpu_fd.set_tsc_khz(u32::MAX / 1000).unwrap();

    let code_segment = kvm_bindings::kvm_segment {
        base: 0,
        limit: 0xffffffff,
        selector: 0x8,
        type_: 0b1010,
        present: 1,
        dpl: 0,
        db: 0,
        s: 1,
        l: 1,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };
    let data_segment = kvm_bindings::kvm_segment {
        base: 0,
        limit: 0xffffffff,
        selector: 0x10,
        type_: 0b0010,
        present: 1,
        dpl: 0,
        db: 1,
        s: 1,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    vcpu_sregs.cs = code_segment;
    vcpu_sregs.ds = data_segment;
    vcpu_sregs.es = data_segment;
    vcpu_sregs.fs = data_segment;
    vcpu_sregs.gs = data_segment;

    // the guest should set up its own gdt
    vcpu_sregs.gdt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };

    // TODO: ensure all needed features are present and disable unneeded ones
    let kvm_cpuid = kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();
    vcpu_fd.set_cpuid2(&kvm_cpuid).unwrap();

    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = start_address;
    let initial_stack = unsafe {
        Box::<[u8; 2 * 0x400 * 0x400], &BuddyAllocator>::new_uninit_in(&allocator).assume_init()
    };
    let stack_start = allocator.to_offset(&*initial_stack);
    Box::leak(initial_stack);

    let (endpoint1, endpoint2) = ringbuffer::make_ring_buffer_pair(1024, &allocator);
    let endpoint_raw = Box::new_in(endpoint2.into_raw_parts(&allocator), &allocator);

    let raw = allocator.clone().into_raw_parts();
    let pa2ka = |p: usize| (p | 0xFFFF800000000000);
    vcpu_regs.rsp = pa2ka((stack_start + (2 * 0x400 * 0x400)) as usize) as u64;
    vcpu_regs.rdi = raw.inner_offset as u64;
    vcpu_regs.rsi = raw.inner_size as u64;
    vcpu_regs.rdx = raw.refcnt_offset as u64;
    vcpu_regs.rcx = raw.refcnt_size as u64;
    vcpu_regs.r8 = allocator.to_offset(Box::into_raw(endpoint_raw)) as u64;
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    let mut record: usize = 0;
    thread::scope(|s| {
        let allocator = &allocator;
        s.spawn(move || {
            let msger = Arc::new(RefCell::new(Messenger::new(endpoint1)));
            let x = 1 as u64;
            let y = 1 as u64;
            let lambdaref = client::create_blob(&msger, ADD_ELF, allocator)
                .and_then(|blobref| client::create_thunk(&msger, blobref))
                .and_then(|thunkref| client::run_thunk(&msger, thunkref))
                .ok()
                .expect("Failed to create lambda");

            let resultref = client::create_blob(&msger, &x.to_ne_bytes(), allocator)
                .and_then(|xref| {
                    client::create_blob(&msger, &y.to_ne_bytes(), allocator)
                        .and_then(move |yref| Ok((xref, yref)))
                })
                .and_then(|(xref, yref)| client::create_tree(&msger, vec![xref, yref], allocator))
                .and_then(|argtreeref| client::apply_lambda(&msger, lambdaref, argtreeref))
                .and_then(|thunkref| client::run_thunk(&msger, thunkref))
                .ok()
                .expect("Failed to get result.");

            mem::drop(resultref);
        });

        loop {
            match vcpu_fd.run().expect("run failed") {
                VcpuExit::IoIn(addr, data) => println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                ),
                VcpuExit::IoOut(addr, data) => match addr {
                    0 => {
                        return ExitCode::from(data[0]);
                    }
                    1 => {
                        record = u32::from_ne_bytes(data.try_into().unwrap()) as usize;
                    }
                    2 => {
                        record |= (u32::from_ne_bytes(data.try_into().unwrap()) as usize) << 32;
                        let record: *const common::LogRecord = allocator.from_offset(record);
                        unsafe {
                            let common::LogRecord {
                                level,
                                target,
                                file,
                                line,
                                module_path,
                                message,
                            } = *record;
                            let level = log::Level::iter().nth(level as usize - 1).unwrap();
                            let target = std::str::from_raw_parts(
                                allocator.from_offset::<u8>(target.0),
                                target.1,
                            );
                            let file = file.map(|x| {
                                std::str::from_raw_parts(allocator.from_offset::<u8>(x.0), x.1)
                            });
                            let module_path = module_path.map(|x| {
                                std::str::from_raw_parts(allocator.from_offset::<u8>(x.0), x.1)
                            });
                            let message = std::str::from_raw_parts(
                                allocator.from_offset::<u8>(message.0),
                                message.1,
                            );
                            log::logger().log(
                                &log::Record::builder()
                                    .level(level)
                                    .target(target)
                                    .file(file)
                                    .line(line)
                                    .module_path(module_path)
                                    .args(format_args!("{}", message))
                                    .build(),
                            );
                        }
                    }
                    0xe9 => {
                        let data = data[0];
                        let c = data as char;
                        print!("{c}");
                    }
                    _ => {
                        println!(
                            "Received an I/O out exit. Address: {:#x}. Data: {:#x}",
                            addr, data[0],
                        );
                    }
                },
                VcpuExit::MmioRead(addr, _) => {
                    println!("Received an MMIO Read Request for the address {:#x}.", addr,);
                }
                VcpuExit::MmioWrite(addr, data) => {
                    println!(
                        "Received an MMIO Write Request to the address {:#x}: {:x}.",
                        addr, data[0]
                    );
                }
                r => panic!("Unexpected exit reason: {:?}", r),
            }
        }
    })
}
