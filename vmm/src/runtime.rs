use std::{
    collections::HashMap,
    io::{self, Read},
    process::ExitCode,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{Scope, ScopedJoinHandle},
    time::{Duration, Instant},
};

use common::BuddyAllocator;
use elf::{endian::AnyEndian, segment::ProgramHeader, ElfBytes};
use kvm_bindings::{
    kvm_userspace_memory_region, CpuId, KVM_MAX_CPUID_ENTRIES,
};
use kvm_ioctls::{IoEventAddress, Kvm, NoDatamatch, VcpuExit, VcpuFd, VmFd};

pub use common::mmap::Mmap;
use libc::EFD_NONBLOCK;
use vmm_sys_util::eventfd::EventFd;

use crate::vhost::VSockBackend;

fn new_cpu<'scope>(
    i: usize,
    scope: &'scope Scope<'scope, '_>,
    vcpu_fd: VcpuFd,
    elf: &'scope ElfBytes<AnyEndian>,
    args: &[u64; 6],
    cpuid: &CpuId,
) -> ScopedJoinHandle<'scope, ()> {
    // set up the CPU in long mode
    let mut vcpu_sregs = vcpu_fd.get_sregs().unwrap();

    let mut pdpt: Box<[u64; 0x200], BuddyAllocator> =
        unsafe { Box::new_zeroed_in(BuddyAllocator).assume_init() };
    for (i, entry) in pdpt.iter_mut().enumerate() {
        *entry = ((i << 30) | 0x183) as u64;
    }

    let mut pml4: Box<[u64; 0x200], BuddyAllocator> =
        unsafe { Box::new_zeroed_in(BuddyAllocator).assume_init() };
    pml4[256] = BuddyAllocator.to_offset(Box::leak(pdpt)) as u64 | 3;
    let pml4 = Box::leak(pml4);

    vcpu_sregs.idt = kvm_bindings::kvm_dtable {
        base: 0,
        limit: 0,
        padding: [0; 3],
    };
    use common::controlreg::*;
    vcpu_sregs.cr0 |= ControlReg0::PG | ControlReg0::PE | ControlReg0::MP | ControlReg0::WP;
    vcpu_sregs.cr0 &= !ControlReg0::EM;
    vcpu_sregs.cr3 = BuddyAllocator.to_offset(pml4) as u64;
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

    vcpu_fd.set_cpuid2(cpuid).unwrap();

    vcpu_fd.set_sregs(&vcpu_sregs).unwrap();

    let mut vcpu_regs = vcpu_fd.get_regs().unwrap();
    vcpu_regs.rip = elf.ehdr.e_entry;
    const STACK_SIZE: usize = 2 * 0x400 * 0x400;
    let initial_stack = unsafe {
        Box::<[u8; STACK_SIZE], BuddyAllocator>::new_uninit_in(BuddyAllocator).assume_init()
    };
    let stack_start = BuddyAllocator.to_offset(&*initial_stack);
    Box::leak(initial_stack);

    let pa2ka = |p: usize| p | 0xFFFF800000000000;
    vcpu_regs.rsp = pa2ka(stack_start + STACK_SIZE) as u64;
    vcpu_regs.rbp = 0;
    vcpu_regs.rdi = args[0];
    vcpu_regs.rsi = args[1];
    vcpu_regs.rdx = args[2];
    vcpu_regs.rcx = args[3];
    vcpu_regs.r8 = args[4];
    vcpu_regs.r8 = args[5];
    vcpu_regs.rflags = 2;
    vcpu_fd.set_regs(&vcpu_regs).unwrap();

    // the Drop impl will wait for the child thread to exit, so we can pretend the allocator
    // lives forever
    let exit = Arc::new(AtomicBool::new(false));
    let flag = exit.clone();
    std::thread::Builder::new()
        .name(format!("Arca vCPU {i}"))
        .spawn_scoped(scope, move || {
            run_cpu(vcpu_fd, elf, flag);
        })
        .unwrap()
}

fn run_cpu(mut vcpu_fd: VcpuFd, elf: &ElfBytes<AnyEndian>, exit: Arc<AtomicBool>) {
    let mut log_record: usize = 0;
    let mut symtab_record: usize = 0;

    let lookup = |target| {
        let (symtab, strtab) = elf
            .symbol_table()
            .expect("could not parse ELF file")
            .expect("could not find symbol table");
        symtab.into_iter().find_map(|symbol| {
            let index = symbol.st_name as usize;
            let addr = symbol.st_value as usize;
            let size = symbol.st_size as usize;
            let end = addr + size;
            if target >= addr && target < end {
                let name = strtab.get(index).unwrap();
                let name = rustc_demangle::demangle(name);
                Some((name.to_string(), addr))
            } else {
                None
            }
        })
    };

    while !exit.load(Ordering::Acquire) {
        vcpu_fd
            .set_mp_state(kvm_bindings::kvm_mp_state {
                mp_state: kvm_bindings::KVM_MP_STATE_RUNNABLE,
            })
            .unwrap();
        let vcpu_exit = vcpu_fd.run().expect("run failed");
        match vcpu_exit {
            VcpuExit::IoIn(addr, data) => match addr {
                0xe9 => {
                    io::stdin().read_exact(data).unwrap();
                }
                addr => println!(
                    "Received an I/O in exit. Address: {:#x}. Data: {:#x}",
                    addr, data[0],
                ),
            },
            VcpuExit::IoOut(addr, data) => match addr {
                0 => {
                    ExitCode::from(data[0]).exit_process();
                }
                1 => {
                    log_record = u32::from_ne_bytes(data.try_into().unwrap()) as usize;
                }
                2 => {
                    log_record |= (u32::from_ne_bytes(data.try_into().unwrap()) as usize) << 32;
                    let record: *const common::LogRecord = BuddyAllocator.from_offset(log_record);
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
                            BuddyAllocator.from_offset::<u8>(target.0),
                            target.1,
                        );
                        let file = file.map(|x| {
                            std::str::from_raw_parts(BuddyAllocator.from_offset::<u8>(x.0), x.1)
                        });
                        let module_path = module_path.map(|x| {
                            std::str::from_raw_parts(BuddyAllocator.from_offset::<u8>(x.0), x.1)
                        });
                        let message = std::str::from_raw_parts(
                            BuddyAllocator.from_offset::<u8>(message.0),
                            message.1,
                        );

                        log::logger().log(
                            &log::Record::builder()
                                .level(level)
                                .target(target)
                                .file(file)
                                .line(line)
                                .module_path(module_path)
                                .args(format_args!("{message}"))
                                .build(),
                        );
                    }
                }
                3 => {
                    symtab_record = u32::from_ne_bytes(data.try_into().unwrap()) as usize;
                }
                4 => {
                    symtab_record |= (u32::from_ne_bytes(data.try_into().unwrap()) as usize) << 32;
                    let record: *mut common::SymtabRecord =
                        BuddyAllocator.from_offset(symtab_record);
                    let record: &mut common::SymtabRecord = unsafe { &mut *record };
                    let target = record.addr;
                    if let Some((name, addr)) = lookup(target) {
                        record.file_len = name.len();
                        let (ptr, cap) = record.file_buffer;
                        let buffer: &mut [u8] = unsafe {
                            core::slice::from_raw_parts_mut(BuddyAllocator.from_offset(ptr), cap)
                        };
                        if name.len() > cap {
                            buffer.copy_from_slice(&name.as_bytes()[..cap]);
                        } else {
                            buffer[..name.len()].copy_from_slice(name.as_bytes());
                        }
                        record.offset = record.addr - addr;
                        record.addr = addr;
                        record.found = true;
                    } else {
                        record.found = false;
                    }
                }
                0xe9 => {
                    let data = data[0];
                    let c = data as char;
                    eprint!("{c}");
                }
                _ => {
                    println!(
                        "Received an I/O out exit. Address: {:#x}. Data: {:#x}",
                        addr, data[0],
                    );
                }
            },
            VcpuExit::MmioRead(addr, _) => {
                println!("Received an MMIO Read Request for the address {addr:#x}.",);
            }
            VcpuExit::MmioWrite(addr, data) => {
                println!(
                    "Received an MMIO Write Request to the address {:#x}: {:x}.",
                    addr, data[0]
                );
            }
            r => {
                let error = format!("{r:?}");
                let regs = vcpu_fd.get_regs().unwrap();
                let rip = regs.rip as usize;
                if let Some((name, _)) = lookup(rip) {
                    panic!("Unexpected exit reason: {error} (in {name}) w/{regs:#x?}");
                } else {
                    panic!("Unexpected exit reason: {error} (@ {rip:#x}) w/{regs:#x?}");
                }
            }
        }
    }
}

pub struct Runtime {
    kvm: Kvm,
    vm: VmFd,
    vsock: VSockBackend,
    cores: usize,
    elf: Arc<[u8]>,
}

impl Runtime {
    pub fn new(cores: usize, ram: usize, elf: Arc<[u8]>) -> Self {
        let kvm = Kvm::new().unwrap();
        let vm = kvm.create_vm().unwrap();
        vm.create_irq_chip().unwrap();

        common::buddy::init(ram);

        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: 0,
            memory_size: BuddyAllocator.len() as u64,
            userspace_addr: BuddyAllocator.base() as u64,
            flags: 0,
        };
        unsafe { vm.set_user_memory_region(mem_region).unwrap() };

        let kick = EventFd::new(EFD_NONBLOCK).unwrap();
        let call = EventFd::new(EFD_NONBLOCK).unwrap();

        let int = EventFd::new(EFD_NONBLOCK).unwrap();

        vm.register_ioevent(&kick, &IoEventAddress::Pio(0xf4), NoDatamatch)
            .unwrap();
        vm.register_irqfd(&call, 0).unwrap();
        vm.register_irqfd(&int, 1).unwrap();

        let mut last_time = None;
        ctrlc::set_handler(move || {
            let now = Instant::now();
            let diff = last_time.map(|last_time| now.duration_since(last_time));
            if let Some(diff) = diff {
                if diff < Duration::from_secs(1) {
                    log::warn!("got ^C^C; forcing immediate shutdown");
                    ExitCode::from(130).exit_process();
                }
            }
            log::info!("got ^C; sending soft shutdown");
            int.write(1).unwrap();
            last_time = Some(now);
        })
        .unwrap();

        let vsock = VSockBackend::new(3, 1024, kick, call).unwrap();

        let mut x = Self {
            kvm,
            vm,
            vsock,
            cores,
            elf: elf.clone(),
        };
        let elf_bytes =
            ElfBytes::<AnyEndian>::minimal_parse(&elf).expect("could not read kernel elf file");
        x.load_elf(&elf_bytes);
        x
    }

    fn load_elf(&mut self, elf: &ElfBytes<AnyEndian>) {
        let segments: Vec<ProgramHeader> = elf
            .segments()
            .expect("could not find ELF segments")
            .iter()
            .collect();

        let align_down = |alignment: usize, value: usize| value & !(alignment - 1);

        let mut reserved = HashMap::new();
        let mut reserve = |addr: usize| -> *mut () {
            let addr = align_down(4096, addr);
            *reserved.entry(addr).or_insert_with(|| {
                let result = BuddyAllocator.reserve_raw(addr, 4096);
                assert!(!result.is_null(), "could not reserve {addr:x}");
                result
            })
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
                    let memsz = segment.p_memsz as usize;
                    let filesz = segment.p_filesz as usize;
                    let end = start + memsz;
                    let ptr = reserve_range(start, end);
                    // let offset = segment.p_offset as usize;
                    let source = &elf.segment_data(&segment).unwrap()[..filesz];
                    unsafe {
                        (&mut (*ptr))[..filesz].copy_from_slice(source);
                    }
                }
                0x60000000.. => {
                    // OS-specific (probably GNU/Linux cruft)
                }
                x => unimplemented!("segment type {x:#x}"),
            }
        }
    }

    pub fn run(&mut self, args: &[usize]) {
        self.vsock.set_running(true).unwrap();
        let elf = ElfBytes::<AnyEndian>::minimal_parse(&self.elf)
            .expect("could not read kernel elf file");

        let allocator_raw = common::buddy::export();
        let allocator_raw =
            Box::into_raw_with_allocator(Box::new_in(allocator_raw, BuddyAllocator)).0;

        // let args = args.to_vec_in(BuddyAllocator);
        let mut inner_args = Vec::new_in(BuddyAllocator);
        let vsock_meta = Box::new_in(self.vsock.metadata(), BuddyAllocator);
        inner_args.push(BuddyAllocator.to_offset(Box::into_raw_with_allocator(vsock_meta).0));
        inner_args.extend_from_slice(args);

        std::thread::scope(|s| {
            let mut cpus = vec![];
            for i in 0..self.cores {
                let vcpu_fd = self.vm.create_vcpu(i as u64).unwrap();
                // TODO: ensure all needed features are present and disable unneeded ones
                let kvm_cpuid = self.kvm.get_supported_cpuid(KVM_MAX_CPUID_ENTRIES).unwrap();

                let mut msrs = kvm_bindings::Msrs::from_entries(&[kvm_bindings::kvm_msr_entry {
                    index: 0xC0000103,
                    ..Default::default()
                }])
                .unwrap();

                vcpu_fd.get_msrs(&mut msrs).unwrap();
                let x = &mut msrs.as_mut_slice()[0];
                x.data = i as u64;
                vcpu_fd.set_msrs(&msrs).unwrap();

                let allocator_raw_offset = BuddyAllocator.to_offset(allocator_raw);
                assert!(!inner_args.is_empty());
                let inner_args_offset = BuddyAllocator.to_offset(inner_args.as_ptr());
                cpus.push(new_cpu(
                    i,
                    s,
                    vcpu_fd,
                    &elf,
                    &[
                        self.cores as u64,
                        allocator_raw_offset as u64,
                        inner_args.len() as u64,
                        inner_args_offset as u64,
                        0,
                        0,
                    ],
                    &kvm_cpuid,
                ));
            }
            for cpu in cpus {
                cpu.join().unwrap();
            }
        });
    }

    pub fn cid(&self) -> u64 {
        self.vsock.cid()
    }
}
